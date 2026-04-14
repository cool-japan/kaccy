//! Hardware Wallet Interface (HWI) integration
//!
//! This module provides integration with hardware wallets (Ledger, Trezor, etc.)
//! for secure key storage and transaction signing.

use crate::error::{BitcoinError, Result};
use base64::{Engine as _, engine::general_purpose};
use bitcoin::{Address, Network, Psbt};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;

/// Supported hardware wallet types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HardwareWalletType {
    /// Ledger hardware wallet
    Ledger,
    /// Trezor hardware wallet
    Trezor,
    /// Coldcard hardware wallet
    Coldcard,
    /// BitBox hardware wallet
    BitBox,
    /// KeepKey hardware wallet
    KeepKey,
}

impl HardwareWalletType {
    /// Get the HWI device type string
    pub fn as_str(&self) -> &'static str {
        match self {
            HardwareWalletType::Ledger => "ledger",
            HardwareWalletType::Trezor => "trezor",
            HardwareWalletType::Coldcard => "coldcard",
            HardwareWalletType::BitBox => "bitbox",
            HardwareWalletType::KeepKey => "keepkey",
        }
    }
}

/// Hardware wallet device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareDevice {
    /// Device type
    pub device_type: HardwareWalletType,
    /// Device model
    pub model: String,
    /// Device fingerprint (master key fingerprint)
    pub fingerprint: String,
    /// Device path (USB path or similar)
    pub path: String,
    /// Whether the device needs a PIN
    pub needs_pin: bool,
    /// Whether the device needs a passphrase
    pub needs_passphrase: bool,
}

/// Hardware wallet configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HwiConfig {
    /// Path to the HWI executable
    pub hwi_path: PathBuf,
    /// Network to use
    pub network: Network,
    /// Timeout for HWI commands (in seconds)
    pub timeout: u64,
}

impl Default for HwiConfig {
    fn default() -> Self {
        Self {
            hwi_path: PathBuf::from("hwi"),
            network: Network::Bitcoin,
            timeout: 30,
        }
    }
}

/// Hardware wallet manager
pub struct HardwareWalletManager {
    config: HwiConfig,
}

impl HardwareWalletManager {
    /// Create a new hardware wallet manager
    pub fn new(config: HwiConfig) -> Self {
        Self { config }
    }

    /// Create a hardware wallet manager with default configuration
    pub fn with_default_config(network: Network) -> Self {
        Self {
            config: HwiConfig {
                network,
                ..Default::default()
            },
        }
    }

    /// Enumerate connected hardware wallets
    pub fn enumerate_devices(&self) -> Result<Vec<HardwareDevice>> {
        let output = Command::new(&self.config.hwi_path)
            .arg("enumerate")
            .output()
            .map_err(|e| BitcoinError::InvalidAddress(format!("Failed to execute HWI: {}", e)))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(BitcoinError::InvalidAddress(format!(
                "HWI enumerate failed: {}",
                error
            )));
        }

        let devices: Vec<HardwareDevice> = serde_json::from_slice(&output.stdout).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to parse HWI output: {}", e))
        })?;

        tracing::info!(count = devices.len(), "Enumerated hardware devices");

        Ok(devices)
    }

    /// Get the master public key (xpub) from a hardware wallet
    pub fn get_xpub(&self, device: &HardwareDevice, derivation_path: &str) -> Result<String> {
        let output = Command::new(&self.config.hwi_path)
            .arg("--device-path")
            .arg(&device.path)
            .arg("getxpub")
            .arg(derivation_path)
            .output()
            .map_err(|e| {
                BitcoinError::InvalidAddress(format!("Failed to execute HWI getxpub: {}", e))
            })?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(BitcoinError::InvalidAddress(format!(
                "HWI getxpub failed: {}",
                error
            )));
        }

        let response: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to parse HWI output: {}", e))
        })?;

        let xpub = response["xpub"]
            .as_str()
            .ok_or_else(|| BitcoinError::InvalidAddress("No xpub in HWI response".to_string()))?
            .to_string();

        tracing::info!(
            device_type = ?device.device_type,
            path = derivation_path,
            "Retrieved xpub from hardware wallet"
        );

        Ok(xpub)
    }

    /// Get a Bitcoin address from a hardware wallet
    pub fn get_address(
        &self,
        device: &HardwareDevice,
        derivation_path: &str,
        show_on_device: bool,
    ) -> Result<Address> {
        let mut cmd = Command::new(&self.config.hwi_path);
        cmd.arg("--device-path")
            .arg(&device.path)
            .arg("displayaddress");

        if !show_on_device {
            cmd.arg("--no-display");
        }

        cmd.arg(derivation_path);

        let output = cmd.output().map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to execute HWI displayaddress: {}", e))
        })?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(BitcoinError::InvalidAddress(format!(
                "HWI displayaddress failed: {}",
                error
            )));
        }

        let response: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to parse HWI output: {}", e))
        })?;

        let address_str = response["address"].as_str().ok_or_else(|| {
            BitcoinError::InvalidAddress("No address in HWI response".to_string())
        })?;

        let address = Address::from_str(address_str)?
            .require_network(self.config.network)
            .map_err(|e| BitcoinError::InvalidAddress(format!("Network mismatch: {}", e)))?;

        tracing::info!(
            device_type = ?device.device_type,
            path = derivation_path,
            show_on_device = show_on_device,
            "Retrieved address from hardware wallet"
        );

        Ok(address)
    }

    /// Sign a PSBT with a hardware wallet
    pub fn sign_psbt(&self, device: &HardwareDevice, psbt: &Psbt) -> Result<Psbt> {
        // Serialize PSBT to base64
        let psbt_base64 = general_purpose::STANDARD.encode(psbt.serialize());

        let output = Command::new(&self.config.hwi_path)
            .arg("--device-path")
            .arg(&device.path)
            .arg("signtx")
            .arg(psbt_base64)
            .output()
            .map_err(|e| {
                BitcoinError::InvalidAddress(format!("Failed to execute HWI signtx: {}", e))
            })?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(BitcoinError::InvalidAddress(format!(
                "HWI signtx failed: {}",
                error
            )));
        }

        let response: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to parse HWI output: {}", e))
        })?;

        let signed_psbt_str = response["psbt"]
            .as_str()
            .ok_or_else(|| BitcoinError::InvalidAddress("No PSBT in HWI response".to_string()))?;

        let psbt_bytes = general_purpose::STANDARD
            .decode(signed_psbt_str)
            .map_err(|e| {
                BitcoinError::InvalidAddress(format!("Failed to decode PSBT base64: {}", e))
            })?;

        let signed_psbt = Psbt::deserialize(&psbt_bytes)?;

        tracing::info!(
            device_type = ?device.device_type,
            "Signed PSBT with hardware wallet"
        );

        Ok(signed_psbt)
    }

    /// Prompt for PIN entry on the hardware wallet
    pub fn prompt_pin(&self, device: &HardwareDevice, pin: &str) -> Result<()> {
        let output = Command::new(&self.config.hwi_path)
            .arg("--device-path")
            .arg(&device.path)
            .arg("promptpin")
            .output()
            .map_err(|e| {
                BitcoinError::InvalidAddress(format!("Failed to execute HWI promptpin: {}", e))
            })?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(BitcoinError::InvalidAddress(format!(
                "HWI promptpin failed: {}",
                error
            )));
        }

        // Send the PIN
        let output = Command::new(&self.config.hwi_path)
            .arg("--device-path")
            .arg(&device.path)
            .arg("sendpin")
            .arg(pin)
            .output()
            .map_err(|e| {
                BitcoinError::InvalidAddress(format!("Failed to execute HWI sendpin: {}", e))
            })?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(BitcoinError::InvalidAddress(format!(
                "HWI sendpin failed: {}",
                error
            )));
        }

        tracing::info!(
            device_type = ?device.device_type,
            "PIN entered successfully"
        );

        Ok(())
    }
}

/// Air-gapped signing workflow for maximum security
pub struct AirGappedSigner {
    /// Directory for exchanging PSBTs
    pub exchange_dir: PathBuf,
}

impl AirGappedSigner {
    /// Create a new air-gapped signer with the given exchange directory
    pub fn new(exchange_dir: PathBuf) -> Self {
        Self { exchange_dir }
    }

    /// Export a PSBT for air-gapped signing
    pub fn export_psbt(&self, psbt: &Psbt, filename: &str) -> Result<PathBuf> {
        let path = self.exchange_dir.join(filename);
        let psbt_base64 = general_purpose::STANDARD.encode(psbt.serialize());

        std::fs::write(&path, psbt_base64).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to write PSBT file: {}", e))
        })?;

        tracing::info!(path = ?path, "Exported PSBT for air-gapped signing");

        Ok(path)
    }

    /// Import a signed PSBT from air-gapped device
    pub fn import_signed_psbt(&self, filename: &str) -> Result<Psbt> {
        let path = self.exchange_dir.join(filename);

        let psbt_base64 = std::fs::read_to_string(&path).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Failed to read PSBT file: {}", e))
        })?;

        let psbt_bytes = general_purpose::STANDARD
            .decode(&psbt_base64)
            .map_err(|e| {
                BitcoinError::InvalidAddress(format!("Failed to decode PSBT base64: {}", e))
            })?;

        let psbt = Psbt::deserialize(&psbt_bytes)?;

        tracing::info!(path = ?path, "Imported signed PSBT from air-gapped device");

        Ok(psbt)
    }

    /// Export a PSBT as QR code for camera-based air-gapped signing
    pub fn export_as_qr(&self, psbt: &Psbt) -> Result<String> {
        // Return the PSBT as base64 for QR encoding
        let psbt_base64 = general_purpose::STANDARD.encode(psbt.serialize());

        tracing::info!("Exported PSBT as QR code data");

        Ok(psbt_base64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardware_wallet_type() {
        assert_eq!(HardwareWalletType::Ledger.as_str(), "ledger");
        assert_eq!(HardwareWalletType::Trezor.as_str(), "trezor");
        assert_eq!(HardwareWalletType::Coldcard.as_str(), "coldcard");
        assert_eq!(HardwareWalletType::BitBox.as_str(), "bitbox");
        assert_eq!(HardwareWalletType::KeepKey.as_str(), "keepkey");
    }

    #[test]
    fn test_hwi_config_default() {
        let config = HwiConfig::default();
        assert_eq!(config.network, Network::Bitcoin);
        assert_eq!(config.timeout, 30);
    }

    #[test]
    fn test_hardware_wallet_manager_creation() {
        let manager = HardwareWalletManager::with_default_config(Network::Testnet);
        assert_eq!(manager.config.network, Network::Testnet);
    }

    #[test]
    fn test_air_gapped_signer_creation() {
        let signer = AirGappedSigner::new(PathBuf::from("/tmp/psbt"));
        assert_eq!(signer.exchange_dir, PathBuf::from("/tmp/psbt"));
    }
}
