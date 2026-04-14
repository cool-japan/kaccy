//! User model

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;

/// Validation error for user-related operations
#[derive(Debug, Clone)]
pub struct ValidationError(pub String);

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ValidationError {}

/// Platform user account
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    /// Unique identifier for this user
    pub user_id: Uuid,
    /// User's email address (used for authentication)
    pub email: String,
    /// Bcrypt/Argon2 hash of the user's password; excluded from serialization
    #[serde(skip_serializing)]
    pub password_hash: String,
    /// Unique handle chosen by the user
    pub username: String,
    /// Human-readable display name shown in the UI
    pub display_name: Option<String>,
    /// Short user biography
    pub bio: Option<String>,
    /// URL to the user's profile picture
    pub avatar_url: Option<String>,
    /// Bitcoin address for withdrawals
    pub btc_withdrawal_address: Option<String>,
    /// Timestamp when the account was created
    pub created_at: DateTime<Utc>,
    /// Know-Your-Customer verification status
    pub kyc_status: KycStatus,
    /// Reputation score accumulated through platform activity
    pub reputation_score: Decimal,
    /// Access role determining permissions
    pub role: UserRole,
}

/// KYC (Know Your Customer) verification status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum KycStatus {
    /// KYC documents submitted but not yet reviewed
    #[default]
    Pending,
    /// KYC review passed; user may access all features
    Verified,
    /// KYC rejected; user must re-submit or contact support
    Rejected,
}

/// Access role for a user account
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum UserRole {
    /// Standard user with trading permissions
    #[default]
    User,
    /// Administrator with elevated platform management permissions
    Admin,
}

/// Public-facing user profile (no sensitive fields)
#[derive(Debug, Serialize, FromRow)]
pub struct UserPublic {
    /// User's unique identifier
    pub user_id: Uuid,
    /// User's email address
    pub email: String,
    /// User's unique handle
    pub username: String,
    /// Human-readable display name
    pub display_name: Option<String>,
    /// Short user biography
    pub bio: Option<String>,
    /// URL to the user's profile picture
    pub avatar_url: Option<String>,
    /// Timestamp when the account was created
    pub created_at: DateTime<Utc>,
    /// KYC verification status
    pub kyc_status: KycStatus,
    /// Reputation score
    pub reputation_score: Decimal,
}

impl From<User> for UserPublic {
    fn from(user: User) -> Self {
        Self {
            user_id: user.user_id,
            email: user.email,
            username: user.username,
            display_name: user.display_name,
            bio: user.bio,
            avatar_url: user.avatar_url,
            created_at: user.created_at,
            kyc_status: user.kyc_status,
            reputation_score: user.reputation_score,
        }
    }
}

impl fmt::Display for User {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "User({}, @{}, {})",
            self.user_id, self.username, self.email
        )
    }
}

impl fmt::Display for KycStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KycStatus::Pending => write!(f, "pending"),
            KycStatus::Verified => write!(f, "verified"),
            KycStatus::Rejected => write!(f, "rejected"),
        }
    }
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserRole::User => write!(f, "user"),
            UserRole::Admin => write!(f, "admin"),
        }
    }
}

/// Request body for new account registration
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    /// Email address for the new account
    pub email: String,
    /// Plaintext password (will be hashed before storage)
    pub password: String,
    /// Desired username
    pub username: String,
}

impl RegisterRequest {
    /// Validate the registration request
    pub fn validate(&self) -> Result<(), ValidationError> {
        // Email validation
        if self.email.is_empty() {
            return Err(ValidationError("Email is required".to_string()));
        }
        if !self.email.contains('@') || !self.email.contains('.') {
            return Err(ValidationError("Invalid email format".to_string()));
        }
        if self.email.len() > 255 {
            return Err(ValidationError("Email is too long".to_string()));
        }

        // Password validation
        if self.password.len() < 8 {
            return Err(ValidationError(
                "Password must be at least 8 characters".to_string(),
            ));
        }
        if self.password.len() > 128 {
            return Err(ValidationError("Password is too long".to_string()));
        }

        // Username validation
        if self.username.is_empty() {
            return Err(ValidationError("Username is required".to_string()));
        }
        if self.username.len() < 3 {
            return Err(ValidationError(
                "Username must be at least 3 characters".to_string(),
            ));
        }
        if self.username.len() > 50 {
            return Err(ValidationError(
                "Username must be at most 50 characters".to_string(),
            ));
        }
        if !self
            .username
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_')
        {
            return Err(ValidationError(
                "Username can only contain letters, numbers, and underscores".to_string(),
            ));
        }

        Ok(())
    }
}

/// Request body for user authentication
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    /// Email address of the account to authenticate
    pub email: String,
    /// Plaintext password to verify against the stored hash
    pub password: String,
}

impl LoginRequest {
    /// Validate the login request
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.email.is_empty() {
            return Err(ValidationError("Email is required".to_string()));
        }
        if self.password.is_empty() {
            return Err(ValidationError("Password is required".to_string()));
        }
        Ok(())
    }
}

/// Response returned after successful authentication
#[derive(Debug, Serialize)]
pub struct AuthResponse {
    /// JWT or session token for subsequent authenticated requests
    pub token: String,
    /// Public profile of the authenticated user
    pub user: UserPublic,
}

/// Request body for updating mutable user profile fields
#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    /// New display name; unchanged if `None`
    pub display_name: Option<String>,
    /// New biography; unchanged if `None`
    pub bio: Option<String>,
    /// New avatar URL (must be HTTPS); unchanged if `None`
    pub avatar_url: Option<String>,
    /// New Bitcoin withdrawal address; unchanged if `None`
    pub btc_withdrawal_address: Option<String>,
}

impl UpdateUserRequest {
    /// Validate the update request
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Some(ref name) = self.display_name {
            if name.len() > 100 {
                return Err(ValidationError(
                    "Display name must be at most 100 characters".to_string(),
                ));
            }
        }
        if let Some(ref bio) = self.bio {
            if bio.len() > 500 {
                return Err(ValidationError(
                    "Bio must be at most 500 characters".to_string(),
                ));
            }
        }
        if let Some(ref url) = self.avatar_url {
            if !url.starts_with("https://") {
                return Err(ValidationError(
                    "Avatar URL must be a valid HTTPS URL".to_string(),
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use uuid::Uuid;

    fn make_user() -> User {
        User {
            user_id: Uuid::new_v4(),
            email: "alice@example.com".to_string(),
            password_hash: "hashed_secret".to_string(),
            username: "alice_42".to_string(),
            display_name: Some("Alice".to_string()),
            bio: Some("Crypto enthusiast".to_string()),
            avatar_url: Some("https://cdn.example.com/avatar.png".to_string()),
            btc_withdrawal_address: None,
            created_at: Utc::now(),
            kyc_status: KycStatus::Verified,
            reputation_score: Decimal::new(9500, 2), // 95.00
            role: UserRole::User,
        }
    }

    // ── RegisterRequest::validate ────────────────────────────────────────────

    #[test]
    fn register_valid_request_passes() {
        let req = RegisterRequest {
            email: "bob@example.com".to_string(),
            password: "securepass1".to_string(),
            username: "bob_dev".to_string(),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn register_empty_email_fails() {
        let req = RegisterRequest {
            email: "".to_string(),
            password: "securepass1".to_string(),
            username: "bob_dev".to_string(),
        };
        let err = req.validate().expect_err("empty email should fail");
        assert!(err.0.contains("Email is required"));
    }

    #[test]
    fn register_email_missing_at_sign_fails() {
        let req = RegisterRequest {
            email: "notanemail.com".to_string(),
            password: "securepass1".to_string(),
            username: "bob_dev".to_string(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn register_email_missing_dot_fails() {
        let req = RegisterRequest {
            email: "bob@nodot".to_string(),
            password: "securepass1".to_string(),
            username: "bob_dev".to_string(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn register_email_too_long_fails() {
        let long_local = "a".repeat(250);
        let email = format!("{}@example.com", long_local);
        let req = RegisterRequest {
            email,
            password: "securepass1".to_string(),
            username: "bob_dev".to_string(),
        };
        let err = req.validate().expect_err("too-long email should fail");
        assert!(err.0.contains("too long"));
    }

    #[test]
    fn register_password_too_short_fails() {
        let req = RegisterRequest {
            email: "bob@example.com".to_string(),
            password: "short".to_string(),
            username: "bob_dev".to_string(),
        };
        let err = req.validate().expect_err("short password should fail");
        assert!(err.0.contains("at least 8 characters"));
    }

    #[test]
    fn register_password_too_long_fails() {
        let req = RegisterRequest {
            email: "bob@example.com".to_string(),
            password: "x".repeat(129),
            username: "bob_dev".to_string(),
        };
        let err = req.validate().expect_err("too-long password should fail");
        assert!(err.0.contains("too long"));
    }

    #[test]
    fn register_empty_username_fails() {
        let req = RegisterRequest {
            email: "bob@example.com".to_string(),
            password: "securepass1".to_string(),
            username: "".to_string(),
        };
        let err = req.validate().expect_err("empty username should fail");
        assert!(err.0.contains("Username is required"));
    }

    #[test]
    fn register_username_too_short_fails() {
        let req = RegisterRequest {
            email: "bob@example.com".to_string(),
            password: "securepass1".to_string(),
            username: "ab".to_string(),
        };
        let err = req.validate().expect_err("short username should fail");
        assert!(err.0.contains("at least 3 characters"));
    }

    #[test]
    fn register_username_too_long_fails() {
        let req = RegisterRequest {
            email: "bob@example.com".to_string(),
            password: "securepass1".to_string(),
            username: "a".repeat(51),
        };
        let err = req.validate().expect_err("long username should fail");
        assert!(err.0.contains("at most 50 characters"));
    }

    #[test]
    fn register_username_invalid_chars_fails() {
        let req = RegisterRequest {
            email: "bob@example.com".to_string(),
            password: "securepass1".to_string(),
            username: "bob@bad!".to_string(),
        };
        let err = req
            .validate()
            .expect_err("username with special chars should fail");
        assert!(err.0.contains("letters, numbers, and underscores"));
    }

    #[test]
    fn register_username_with_underscore_passes() {
        let req = RegisterRequest {
            email: "bob@example.com".to_string(),
            password: "securepass1".to_string(),
            username: "bob_dev_42".to_string(),
        };
        assert!(req.validate().is_ok());
    }

    // ── LoginRequest::validate ───────────────────────────────────────────────

    #[test]
    fn login_valid_request_passes() {
        let req = LoginRequest {
            email: "alice@example.com".to_string(),
            password: "mypassword".to_string(),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn login_empty_email_fails() {
        let req = LoginRequest {
            email: "".to_string(),
            password: "mypassword".to_string(),
        };
        let err = req.validate().expect_err("empty email should fail login");
        assert!(err.0.contains("Email is required"));
    }

    #[test]
    fn login_empty_password_fails() {
        let req = LoginRequest {
            email: "alice@example.com".to_string(),
            password: "".to_string(),
        };
        let err = req
            .validate()
            .expect_err("empty password should fail login");
        assert!(err.0.contains("Password is required"));
    }

    // ── UpdateUserRequest::validate ──────────────────────────────────────────

    #[test]
    fn update_all_none_passes() {
        let req = UpdateUserRequest {
            display_name: None,
            bio: None,
            avatar_url: None,
            btc_withdrawal_address: None,
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn update_display_name_too_long_fails() {
        let req = UpdateUserRequest {
            display_name: Some("x".repeat(101)),
            bio: None,
            avatar_url: None,
            btc_withdrawal_address: None,
        };
        let err = req.validate().expect_err("long display_name should fail");
        assert!(err.0.contains("at most 100 characters"));
    }

    #[test]
    fn update_bio_too_long_fails() {
        let req = UpdateUserRequest {
            display_name: None,
            bio: Some("b".repeat(501)),
            avatar_url: None,
            btc_withdrawal_address: None,
        };
        let err = req.validate().expect_err("long bio should fail");
        assert!(err.0.contains("at most 500 characters"));
    }

    #[test]
    fn update_avatar_url_non_https_fails() {
        let req = UpdateUserRequest {
            display_name: None,
            bio: None,
            avatar_url: Some("http://cdn.example.com/img.png".to_string()),
            btc_withdrawal_address: None,
        };
        let err = req.validate().expect_err("http avatar URL should fail");
        assert!(err.0.contains("HTTPS"));
    }

    #[test]
    fn update_avatar_url_https_passes() {
        let req = UpdateUserRequest {
            display_name: None,
            bio: None,
            avatar_url: Some("https://cdn.example.com/img.png".to_string()),
            btc_withdrawal_address: None,
        };
        assert!(req.validate().is_ok());
    }

    // ── Display impls ────────────────────────────────────────────────────────

    #[test]
    fn user_display_format_is_correct() {
        let user = make_user();
        let s = format!("{}", user);
        assert!(s.contains("User("));
        assert!(s.contains("@alice_42"));
        assert!(s.contains("alice@example.com"));
    }

    #[test]
    fn kyc_status_display_values() {
        assert_eq!(KycStatus::Pending.to_string(), "pending");
        assert_eq!(KycStatus::Verified.to_string(), "verified");
        assert_eq!(KycStatus::Rejected.to_string(), "rejected");
    }

    #[test]
    fn user_role_display_values() {
        assert_eq!(UserRole::User.to_string(), "user");
        assert_eq!(UserRole::Admin.to_string(), "admin");
    }

    // ── Default impls ────────────────────────────────────────────────────────

    #[test]
    fn kyc_status_default_is_pending() {
        assert_eq!(KycStatus::default(), KycStatus::Pending);
    }

    #[test]
    fn user_role_default_is_user() {
        assert_eq!(UserRole::default(), UserRole::User);
    }

    // ── From<User> for UserPublic ────────────────────────────────────────────

    #[test]
    fn user_public_from_user_drops_password_hash() {
        let user = make_user();
        let uid = user.user_id;
        let public: UserPublic = user.into();

        assert_eq!(public.user_id, uid);
        assert_eq!(public.email, "alice@example.com");
        assert_eq!(public.username, "alice_42");
        assert_eq!(public.kyc_status, KycStatus::Verified);
        // password_hash is not a field on UserPublic — confirmed absent by type
    }

    #[test]
    fn user_public_preserves_optional_fields() {
        let user = make_user();
        let public: UserPublic = user.into();

        assert_eq!(public.display_name, Some("Alice".to_string()));
        assert_eq!(public.bio, Some("Crypto enthusiast".to_string()));
        assert_eq!(
            public.avatar_url,
            Some("https://cdn.example.com/avatar.png".to_string())
        );
    }

    #[test]
    fn user_public_preserves_none_optional_fields() {
        let mut user = make_user();
        user.display_name = None;
        user.bio = None;
        user.avatar_url = None;
        let public: UserPublic = user.into();

        assert!(public.display_name.is_none());
        assert!(public.bio.is_none());
        assert!(public.avatar_url.is_none());
    }
}
