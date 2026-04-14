//! Shared types for SDK generation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported SDK languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SdkLanguage {
    /// Generate a TypeScript/JavaScript SDK
    TypeScript,
    /// Generate a Python SDK
    Python,
    /// Generate a Rust SDK
    Rust,
    /// Generate a Go SDK
    Go,
}

impl std::fmt::Display for SdkLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SdkLanguage::TypeScript => write!(f, "typescript"),
            SdkLanguage::Python => write!(f, "python"),
            SdkLanguage::Rust => write!(f, "rust"),
            SdkLanguage::Go => write!(f, "go"),
        }
    }
}

/// SDK generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkConfig {
    /// Target language
    pub language: SdkLanguage,
    /// Package name
    pub package_name: String,
    /// Package version
    pub version: String,
    /// API base URL
    pub base_url: String,
    /// Additional configuration
    pub options: HashMap<String, String>,
}

/// Generated SDK files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedSdk {
    /// Language
    pub language: SdkLanguage,
    /// Generated files (filename -> content)
    pub files: HashMap<String, String>,
    /// Package metadata
    pub metadata: SdkMetadata,
}

/// SDK package metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkMetadata {
    /// Package name
    pub name: String,
    /// Version
    pub version: String,
    /// Description
    pub description: String,
    /// Dependencies
    pub dependencies: Vec<String>,
}
