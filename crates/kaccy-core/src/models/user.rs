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

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub user_id: Uuid,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub username: String,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub btc_withdrawal_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub kyc_status: KycStatus,
    pub reputation_score: Decimal,
    pub role: UserRole,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum KycStatus {
    #[default]
    Pending,
    Verified,
    Rejected,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum UserRole {
    #[default]
    User,
    Admin,
}

#[derive(Debug, Serialize, FromRow)]
pub struct UserPublic {
    pub user_id: Uuid,
    pub email: String,
    pub username: String,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub kyc_status: KycStatus,
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

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
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

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
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

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserPublic,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
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
