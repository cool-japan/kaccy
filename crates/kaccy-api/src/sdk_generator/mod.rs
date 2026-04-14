//! SDK generator for multiple programming languages
//!
//! Generates client libraries for TypeScript, Python, and Rust from OpenAPI spec

mod go_gen;
mod python;
mod rust_gen;
pub mod types;
mod typescript;

pub use go_gen::GoGenerator;
pub use python::PythonGenerator;
pub use rust_gen::RustGenerator;
pub use types::{GeneratedSdk, SdkConfig, SdkLanguage, SdkMetadata};
pub use typescript::TypeScriptGenerator;

/// Generate SDK for a given language
pub fn generate_sdk(config: SdkConfig) -> GeneratedSdk {
    match config.language {
        SdkLanguage::TypeScript => TypeScriptGenerator::generate(&config),
        SdkLanguage::Python => PythonGenerator::generate(&config),
        SdkLanguage::Rust => RustGenerator::generate(&config),
        SdkLanguage::Go => GoGenerator::generate(&config),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_config(language: SdkLanguage) -> SdkConfig {
        SdkConfig {
            language,
            package_name: "kaccy-sdk".to_string(),
            version: "1.0.0".to_string(),
            base_url: "http://localhost:3000".to_string(),
            options: HashMap::new(),
        }
    }

    #[test]
    fn test_typescript_generation() {
        let config = test_config(SdkLanguage::TypeScript);
        let sdk = TypeScriptGenerator::generate(&config);

        assert_eq!(sdk.language, SdkLanguage::TypeScript);
        assert!(sdk.files.contains_key("client.ts"));
        assert!(sdk.files.contains_key("types.ts"));
        assert!(sdk.files.contains_key("package.json"));
        assert!(sdk.files.contains_key("README.md"));
    }

    #[test]
    fn test_python_generation() {
        let config = test_config(SdkLanguage::Python);
        let sdk = PythonGenerator::generate(&config);

        assert_eq!(sdk.language, SdkLanguage::Python);
        assert!(sdk.files.contains_key("client.py"));
        assert!(sdk.files.contains_key("models.py"));
        assert!(sdk.files.contains_key("setup.py"));
        assert!(sdk.files.contains_key("README.md"));
    }

    #[test]
    fn test_rust_generation() {
        let config = test_config(SdkLanguage::Rust);
        let sdk = RustGenerator::generate(&config);

        assert_eq!(sdk.language, SdkLanguage::Rust);
        assert!(sdk.files.contains_key("lib.rs"));
        assert!(sdk.files.contains_key("client.rs"));
        assert!(sdk.files.contains_key("types.rs"));
        assert!(sdk.files.contains_key("Cargo.toml"));
        assert!(sdk.files.contains_key("README.md"));
    }

    #[test]
    fn test_go_generation() {
        let config = test_config(SdkLanguage::Go);
        let sdk = GoGenerator::generate(&config);

        assert_eq!(sdk.language, SdkLanguage::Go);
        assert!(sdk.files.contains_key("client.go"));
        assert!(sdk.files.contains_key("types.go"));
        assert!(sdk.files.contains_key("errors.go"));
        assert!(sdk.files.contains_key("go.mod"));
        assert!(sdk.files.contains_key("README.md"));
    }

    #[test]
    fn test_sdk_language_display() {
        assert_eq!(SdkLanguage::TypeScript.to_string(), "typescript");
        assert_eq!(SdkLanguage::Python.to_string(), "python");
        assert_eq!(SdkLanguage::Rust.to_string(), "rust");
        assert_eq!(SdkLanguage::Go.to_string(), "go");
    }
}
