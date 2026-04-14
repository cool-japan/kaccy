//! Multi-party coordination tests for MuSig2, FROST, and multisig workflows.
//!
//! Tests Schnorr signing, batch verification, Taproot address parsing,
//! and descriptor serialization — all without any network connection.

use kaccy_bitcoin::{
    DescriptorImportRequest, SchnorrKeyPair, SchnorrSigner, SchnorrUtils, TaprootAddressInfo,
};

/// Known mainnet Taproot test vector (BIP 341).
const MAINNET_TAPROOT: &str = "bc1p5d7rjq7g6rdk2yhzks9smlaqtedr4dekq08ge8ztwac72sfr9rusxg3297";

#[test]
fn test_schnorr_sign_verify() {
    let kp = SchnorrKeyPair::generate().expect("generate key pair");
    let message = SchnorrUtils::message_from_bytes(b"multi-party coordination message");

    let signature =
        SchnorrSigner::sign(kp.secret_key_bytes(), &message).expect("signing should succeed");

    assert_eq!(signature.len(), 64);

    SchnorrSigner::verify(&kp.public_key(), &message, &signature)
        .expect("verification should succeed");
}

#[test]
fn test_schnorr_batch_verify() {
    let mut items: Vec<([u8; 32], [u8; 32], [u8; 64])> = Vec::with_capacity(3);

    for i in 0..3u8 {
        let kp = SchnorrKeyPair::generate().expect("generate");
        let msg = SchnorrUtils::message_from_bytes(&[i * 10, i * 20, i * 30]);
        let sig = SchnorrSigner::sign(kp.secret_key_bytes(), &msg).expect("sign");
        items.push((kp.public_key(), msg, sig));
    }

    // All 3 should verify successfully
    SchnorrSigner::batch_verify(&items).expect("batch verification of 3 valid signatures");
}

#[test]
fn test_taproot_address_info_parse() {
    let info = TaprootAddressInfo::from_address(MAINNET_TAPROOT)
        .expect("parse should succeed for known Taproot address");

    assert_eq!(info.hrp, "bc", "mainnet Taproot must use 'bc' HRP");
    assert_eq!(info.witness_version, 1, "Taproot uses witness version 1");
    assert_eq!(info.program.len(), 32, "Taproot program must be 32 bytes");
    assert!(info.is_mainnet, "should be mainnet");
    assert!(info.is_taproot(), "is_taproot() should return true");
}

#[test]
fn test_descriptor_import_request_serialization() {
    let req = DescriptorImportRequest {
        descriptor: "tr([d34db33f/86h/0h/0h]xpub6BgBgsespWvERF3LHQu6CnqdvfEvtMcQjYrcRzx53QJjSxarj2afYWcLteoGVky77GA3SoZgZnzm5pMGi5wnv3FXJHBR39WVWTPL1PLtBeEg/0/*)#czjn4tqx".to_string(),
        timestamp: "now".to_string(),
        range: Some([0, 1000]),
        label: Some("taproot-wallet".to_string()),
        watch_only: true,
        active: true,
    };

    // Serialize to JSON
    let json_str = serde_json::to_string(&req).expect("serialize");

    // Deserialize back
    let back: DescriptorImportRequest = serde_json::from_str(&json_str).expect("deserialize");

    assert_eq!(req.descriptor, back.descriptor);
    assert_eq!(req.timestamp, back.timestamp);
    assert_eq!(req.range, back.range);
    assert_eq!(req.label, back.label);
    assert_eq!(req.watch_only, back.watch_only);
    assert_eq!(req.active, back.active);
}
