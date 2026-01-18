//! Two-Factor Authentication (2FA) routes using TOTP

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use totp_rs::{Algorithm, Secret, TOTP};
use uuid::Uuid;

use crate::{
    error::{ApiError, Result, ValidationBuilder},
    middleware::Claims,
    state::AppState,
};

/// Setup 2FA request
#[derive(Debug, Deserialize)]
pub struct Setup2FARequest {}

/// Setup 2FA response
#[derive(Debug, Serialize)]
pub struct Setup2FAResponse {
    /// Secret key (base32 encoded) - user should store this securely
    pub secret: String,
    /// QR code data URL for easy scanning
    pub qr_code: String,
    /// Backup codes for recovery (10 codes)
    pub backup_codes: Vec<String>,
}

/// Enable 2FA request
#[derive(Debug, Deserialize)]
pub struct Enable2FARequest {
    /// TOTP code to verify setup
    pub code: String,
}

/// Disable 2FA request
#[derive(Debug, Deserialize)]
pub struct Disable2FARequest {
    /// TOTP code or backup code
    pub code: String,
}

/// Verify 2FA request (during login)
#[derive(Debug, Deserialize)]
pub struct Verify2FARequest {
    /// User ID
    pub user_id: Uuid,
    /// TOTP code or backup code
    pub code: String,
}

/// 2FA status response
#[derive(Debug, Serialize)]
pub struct TwoFAStatusResponse {
    /// Whether 2FA is enabled
    pub enabled: bool,
    /// Number of remaining backup codes
    pub backup_codes_remaining: Option<i32>,
}

/// Generate TOTP secret and QR code
fn generate_totp_setup(user_email: &str, issuer: &str) -> Result<(String, String, TOTP)> {
    // Generate a random secret (32 bytes = 256 bits)
    use rand::Rng;
    let mut rng = rand::rng();
    let secret_bytes: Vec<u8> = (0..32).map(|_| rng.random_range(0..256) as u8).collect();

    let secret = Secret::Raw(secret_bytes.clone());
    let secret_b32 = secret.to_encoded().to_string();

    // Create TOTP instance
    let totp = TOTP::new(
        Algorithm::SHA1,
        6,  // 6 digits
        1,  // 1 step (30 seconds)
        30, // 30 second period
        secret_bytes,
        Some(issuer.to_string()),
        user_email.to_string(),
    )
    .map_err(|e| ApiError::InternalWithMessage(format!("Failed to create TOTP: {}", e)))?;

    // Generate QR code
    let qr_code_url = totp
        .get_qr_base64()
        .map_err(|e| ApiError::InternalWithMessage(format!("Failed to generate QR code: {}", e)))?;

    Ok((secret_b32, qr_code_url, totp))
}

/// Generate backup codes
fn generate_backup_codes() -> Vec<String> {
    use rand::Rng;
    let mut rng = rand::rng();

    (0..10)
        .map(|_| {
            let code: u32 = rng.random_range(100000..1000000);
            format!("{:06}", code)
        })
        .collect()
}

/// Verify TOTP code
#[allow(dead_code)]
fn verify_totp_code(secret: &str, code: &str) -> Result<bool> {
    let secret_bytes = Secret::Encoded(secret.to_string())
        .to_bytes()
        .map_err(|e| ApiError::InternalWithMessage(format!("Invalid secret: {}", e)))?;

    let totp = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret_bytes,
        None,
        "user".to_string(),
    )
    .map_err(|e| ApiError::InternalWithMessage(format!("Failed to create TOTP: {}", e)))?;

    totp.check_current(code)
        .map_err(|e| ApiError::InternalWithMessage(format!("Failed to verify TOTP: {}", e)))
}

/// Setup 2FA (generate secret and QR code)
pub async fn setup_2fa(
    State(_state): State<AppState>,
    claims: Claims,
) -> Result<Json<Setup2FAResponse>> {
    // In production, get user email from database
    let user_email = format!("user-{}", claims.sub);
    let issuer = "Kaccy";

    let (secret, qr_code, _totp) = generate_totp_setup(&user_email, issuer)?;
    let backup_codes = generate_backup_codes();

    // In production, store the secret and backup codes in the database as pending
    // Only activate after verification

    Ok(Json(Setup2FAResponse {
        secret,
        qr_code,
        backup_codes,
    }))
}

/// Enable 2FA (verify and activate)
pub async fn enable_2fa(
    State(_state): State<AppState>,
    claims: Claims,
    Json(req): Json<Enable2FARequest>,
) -> Result<impl IntoResponse> {
    // Validation
    let mut errors = ValidationBuilder::new();

    if req.code.len() != 6 || !req.code.chars().all(|c| c.is_ascii_digit()) {
        errors = errors.field("code", "TOTP code must be 6 digits");
    }

    if errors.has_errors() {
        return errors.into_result();
    }

    // In production:
    // 1. Get pending 2FA secret from database
    // 2. Verify the TOTP code
    // 3. If valid, activate 2FA and store backup codes
    // 4. Clear pending setup

    // For now, just return success
    let _user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| ApiError::Auth("Invalid user ID in token".to_string()))?;

    // Example verification (in production, use secret from database)
    // verify_totp_code(&secret, &req.code)?;

    Ok((StatusCode::OK, "2FA enabled successfully"))
}

/// Disable 2FA
pub async fn disable_2fa(
    State(_state): State<AppState>,
    claims: Claims,
    Json(req): Json<Disable2FARequest>,
) -> Result<impl IntoResponse> {
    // Validation
    let mut errors = ValidationBuilder::new();

    if req.code.is_empty() {
        errors = errors.field("code", "Code is required");
    }

    if errors.has_errors() {
        return errors.into_result();
    }

    let _user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| ApiError::Auth("Invalid user ID in token".to_string()))?;

    // In production:
    // 1. Get 2FA secret from database
    // 2. Verify TOTP code or check backup code
    // 3. If valid, disable 2FA and clear secrets

    Ok((StatusCode::OK, "2FA disabled successfully"))
}

/// Get 2FA status
pub async fn get_2fa_status(
    State(_state): State<AppState>,
    claims: Claims,
) -> Result<Json<TwoFAStatusResponse>> {
    let _user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| ApiError::Auth("Invalid user ID in token".to_string()))?;

    // In production: query database for 2FA status
    Ok(Json(TwoFAStatusResponse {
        enabled: false,
        backup_codes_remaining: None,
    }))
}

/// Verify 2FA code (called during login)
pub async fn verify_2fa_code(
    State(_state): State<AppState>,
    Json(req): Json<Verify2FARequest>,
) -> Result<impl IntoResponse> {
    // Validation
    let mut errors = ValidationBuilder::new();

    if req.code.is_empty() {
        errors = errors.field("code", "Code is required");
    }

    if errors.has_errors() {
        return errors.into_result();
    }

    // In production:
    // 1. Get 2FA secret from database for user
    // 2. Try to verify as TOTP code
    // 3. If TOTP fails, check backup codes
    // 4. If backup code used, mark it as consumed
    // 5. Return success/failure

    Ok((StatusCode::OK, "2FA verified"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_backup_codes() {
        let codes = generate_backup_codes();
        assert_eq!(codes.len(), 10);

        for code in &codes {
            assert_eq!(code.len(), 6);
            assert!(code.chars().all(|c| c.is_ascii_digit()));
        }
    }

    #[test]
    fn test_totp_generation() {
        let result = generate_totp_setup("test@example.com", "Kaccy");
        assert!(result.is_ok());

        let (secret, qr_code, _totp) = result.unwrap();
        assert!(!secret.is_empty());
        // QR code should not be empty
        assert!(!qr_code.is_empty());
    }

    #[test]
    fn test_totp_verification() {
        // Generate a TOTP setup
        let (secret, _qr, totp) = generate_totp_setup("test@example.com", "Kaccy").unwrap();

        // Generate current code
        let current_code = totp.generate_current().unwrap();

        // Verify it
        let result = verify_totp_code(&secret, &current_code);
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Verify invalid code
        let result = verify_totp_code(&secret, "000000");
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }
}
