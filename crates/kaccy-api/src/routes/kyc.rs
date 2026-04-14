//! KYC (Know Your Customer) verification routes
//!
//! These routes handle user identity verification for regulatory compliance.

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::error::{ApiError, Result};
use crate::state::AppState;

/// KYC verification status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KycStatus {
    /// Not started
    NotStarted,
    /// Documents submitted, pending review
    Pending,
    /// Under manual review
    InReview,
    /// Verified successfully
    Verified,
    /// Verification rejected
    Rejected,
    /// Verification expired (needs renewal)
    Expired,
}

impl KycStatus {
    /// Convert a database string representation to a KycStatus
    pub fn from_db_string(s: &str) -> Self {
        match s {
            "not_started" => KycStatus::NotStarted,
            "pending" => KycStatus::Pending,
            "in_review" => KycStatus::InReview,
            "verified" => KycStatus::Verified,
            "rejected" => KycStatus::Rejected,
            "expired" => KycStatus::Expired,
            _ => KycStatus::NotStarted,
        }
    }

    /// Convert this KycStatus to its database string representation
    pub fn to_db_string(&self) -> &'static str {
        match self {
            KycStatus::NotStarted => "not_started",
            KycStatus::Pending => "pending",
            KycStatus::InReview => "in_review",
            KycStatus::Verified => "verified",
            KycStatus::Rejected => "rejected",
            KycStatus::Expired => "expired",
        }
    }
}

/// KYC verification level
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KycLevel {
    /// No KYC - limited features
    None,
    /// Basic verification (email/phone)
    Basic,
    /// Standard verification (ID + selfie)
    Standard,
    /// Enhanced verification (proof of address)
    Enhanced,
}

impl KycLevel {
    /// Convert a database string representation to a KycLevel
    pub fn from_db_string(s: &str) -> Self {
        match s {
            "none" => KycLevel::None,
            "basic" => KycLevel::Basic,
            "standard" => KycLevel::Standard,
            "enhanced" => KycLevel::Enhanced,
            _ => KycLevel::None,
        }
    }

    /// Convert this KycLevel to its database string representation
    pub fn to_db_string(&self) -> &'static str {
        match self {
            KycLevel::None => "none",
            KycLevel::Basic => "basic",
            KycLevel::Standard => "standard",
            KycLevel::Enhanced => "enhanced",
        }
    }

    /// Get trading limits for this KYC level (in BTC)
    pub fn daily_limit_btc(&self) -> f64 {
        match self {
            KycLevel::None => 0.01,     // ~$1000
            KycLevel::Basic => 0.1,     // ~$10,000
            KycLevel::Standard => 1.0,  // ~$100,000
            KycLevel::Enhanced => 10.0, // ~$1,000,000
        }
    }
}

/// KYC application record
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct KycApplication {
    /// Unique KYC application ID
    pub kyc_id: Uuid,
    /// ID of the user who submitted the application
    pub user_id: Uuid,
    /// Current status (pending, in_review, verified, rejected, expired)
    pub status: String,
    /// KYC verification level (none, basic, standard, enhanced)
    pub level: String,
    /// Full legal name of the applicant
    pub full_name: Option<String>,
    /// Date of birth (YYYY-MM-DD)
    pub date_of_birth: Option<String>,
    /// ISO 2-letter country code
    pub country: Option<String>,
    /// Type of identity document (passport, drivers_license, national_id)
    pub document_type: Option<String>,
    /// Document number
    pub document_number: Option<String>,
    /// Document expiry date
    pub document_expiry: Option<String>,
    /// HTTPS URL to selfie photo
    pub selfie_url: Option<String>,
    /// HTTPS URL to front of identity document
    pub document_front_url: Option<String>,
    /// HTTPS URL to back of identity document
    pub document_back_url: Option<String>,
    /// HTTPS URL to proof of address document
    pub proof_of_address_url: Option<String>,
    /// Reason for rejection (if rejected)
    pub rejection_reason: Option<String>,
    /// ID of admin who reviewed the application
    pub reviewed_by: Option<Uuid>,
    /// Timestamp when application was reviewed
    pub reviewed_at: Option<DateTime<Utc>>,
    /// Timestamp when verification was confirmed
    pub verified_at: Option<DateTime<Utc>>,
    /// Timestamp when verification expires
    pub expires_at: Option<DateTime<Utc>>,
    /// Timestamp when application was created
    pub created_at: DateTime<Utc>,
    /// Timestamp of last update
    pub updated_at: DateTime<Utc>,
}

/// KYC application response (public-safe version)
#[derive(Debug, Clone, Serialize)]
pub struct KycApplicationResponse {
    /// Unique KYC application ID
    pub kyc_id: Uuid,
    /// ID of the user who submitted the application
    pub user_id: Uuid,
    /// Current KYC verification status
    pub status: KycStatus,
    /// KYC verification level
    pub level: KycLevel,
    /// Full legal name
    pub full_name: Option<String>,
    /// ISO 2-letter country code
    pub country: Option<String>,
    /// Reason for rejection (if rejected)
    pub rejection_reason: Option<String>,
    /// Timestamp when verification was confirmed
    pub verified_at: Option<DateTime<Utc>>,
    /// Timestamp when verification expires
    pub expires_at: Option<DateTime<Utc>>,
    /// Timestamp when application was created
    pub created_at: DateTime<Utc>,
}

impl From<KycApplication> for KycApplicationResponse {
    fn from(app: KycApplication) -> Self {
        Self {
            kyc_id: app.kyc_id,
            user_id: app.user_id,
            status: KycStatus::from_db_string(&app.status),
            level: KycLevel::from_db_string(&app.level),
            full_name: app.full_name,
            country: app.country,
            rejection_reason: app.rejection_reason,
            verified_at: app.verified_at,
            expires_at: app.expires_at,
            created_at: app.created_at,
        }
    }
}

/// Request to submit KYC application
#[derive(Debug, Clone, Deserialize)]
pub struct SubmitKycRequest {
    /// Full legal name of the applicant
    pub full_name: String,
    /// Date of birth (YYYY-MM-DD format)
    pub date_of_birth: String,
    /// ISO 2-letter country code
    pub country: String,
    /// Type of identity document (passport, drivers_license, national_id)
    pub document_type: String,
    /// Document identification number
    pub document_number: String,
    /// Document expiry date
    pub document_expiry: Option<String>,
    /// HTTPS URL to selfie photo
    pub selfie_url: String,
    /// HTTPS URL to front of identity document
    pub document_front_url: String,
    /// HTTPS URL to back of identity document
    pub document_back_url: Option<String>,
    /// HTTPS URL to proof of address document (required for enhanced level)
    pub proof_of_address_url: Option<String>,
}

impl SubmitKycRequest {
    /// Validate that the required KYC fields are present and well-formed
    pub fn validate(&self) -> Result<()> {
        if self.full_name.is_empty() {
            return Err(ApiError::Validation("Full name is required".to_string()));
        }
        if self.date_of_birth.is_empty() {
            return Err(ApiError::Validation(
                "Date of birth is required".to_string(),
            ));
        }
        if self.country.len() != 2 {
            return Err(ApiError::Validation(
                "Country must be a 2-letter ISO code".to_string(),
            ));
        }
        if !["passport", "drivers_license", "national_id"].contains(&self.document_type.as_str()) {
            return Err(ApiError::Validation("Invalid document type".to_string()));
        }
        if self.selfie_url.is_empty() || !self.selfie_url.starts_with("https://") {
            return Err(ApiError::Validation(
                "Valid HTTPS selfie URL is required".to_string(),
            ));
        }
        if self.document_front_url.is_empty() || !self.document_front_url.starts_with("https://") {
            return Err(ApiError::Validation(
                "Valid HTTPS document front URL is required".to_string(),
            ));
        }
        Ok(())
    }

    /// Determine KYC level based on submitted documents
    pub fn determine_level(&self) -> KycLevel {
        if self.proof_of_address_url.is_some() {
            KycLevel::Enhanced
        } else {
            KycLevel::Standard
        }
    }
}

/// Admin verification request
#[derive(Debug, Clone, Deserialize)]
pub struct VerifyKycRequest {
    /// Whether to approve or reject the application
    pub approved: bool,
    /// Reason for rejection (required when rejecting)
    pub rejection_reason: Option<String>,
}

/// KYC queue query params
#[derive(Debug, Clone, Deserialize)]
pub struct KycQueueQuery {
    /// Filter by status (default: pending)
    pub status: Option<String>,
    /// Page number (1-indexed)
    pub page: Option<u32>,
    /// Items per page (max 100)
    pub limit: Option<u32>,
}

/// KYC queue response
#[derive(Debug, Clone, Serialize)]
pub struct KycQueueResponse {
    /// List of KYC applications
    pub applications: Vec<KycApplicationResponse>,
    /// Total number of matching applications
    pub total: i64,
    /// Current page number
    pub page: u32,
    /// Items per page
    pub limit: u32,
}

// ============== User Routes ==============

/// Get current user's KYC status
pub async fn get_my_kyc(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<KycApplicationResponse>> {
    let application: Option<KycApplication> = sqlx::query_as(
        "SELECT * FROM kyc_applications WHERE user_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?;

    match application {
        Some(app) => Ok(Json(app.into())),
        None => {
            // Return a "not started" response
            Ok(Json(KycApplicationResponse {
                kyc_id: Uuid::nil(),
                user_id,
                status: KycStatus::NotStarted,
                level: KycLevel::None,
                full_name: None,
                country: None,
                rejection_reason: None,
                verified_at: None,
                expires_at: None,
                created_at: Utc::now(),
            }))
        }
    }
}

/// Submit KYC application
pub async fn submit_kyc(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<SubmitKycRequest>,
) -> Result<Json<KycApplicationResponse>> {
    payload.validate()?;

    // Check if user already has pending KYC
    let existing: Option<(String,)> = sqlx::query_as(
        "SELECT status FROM kyc_applications WHERE user_id = $1 AND status IN ('pending', 'in_review')",
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?;

    if existing.is_some() {
        return Err(ApiError::Validation(
            "You already have a pending KYC application".to_string(),
        ));
    }

    let level = payload.determine_level();

    let application: KycApplication = sqlx::query_as(
        r#"
        INSERT INTO kyc_applications
            (user_id, status, level, full_name, date_of_birth, country,
             document_type, document_number, document_expiry,
             selfie_url, document_front_url, document_back_url, proof_of_address_url)
        VALUES ($1, 'pending', $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        RETURNING *
        "#,
    )
    .bind(user_id)
    .bind(level.to_db_string())
    .bind(&payload.full_name)
    .bind(&payload.date_of_birth)
    .bind(&payload.country)
    .bind(&payload.document_type)
    .bind(&payload.document_number)
    .bind(&payload.document_expiry)
    .bind(&payload.selfie_url)
    .bind(&payload.document_front_url)
    .bind(&payload.document_back_url)
    .bind(&payload.proof_of_address_url)
    .fetch_one(&state.pool)
    .await?;

    tracing::info!(
        user_id = %user_id,
        kyc_id = %application.kyc_id,
        level = ?level,
        "KYC application submitted"
    );

    Ok(Json(application.into()))
}

// ============== Admin Routes ==============

/// Get KYC queue (admin)
pub async fn get_kyc_queue(
    State(state): State<AppState>,
    Query(query): Query<KycQueueQuery>,
) -> Result<Json<KycQueueResponse>> {
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).min(100);
    let offset = (page - 1) * limit;

    let status_filter = query.status.as_deref().unwrap_or("pending");

    let applications: Vec<KycApplication> = sqlx::query_as(
        r#"
        SELECT * FROM kyc_applications
        WHERE status = $1
        ORDER BY created_at ASC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(status_filter)
    .bind(i64::from(limit))
    .bind(i64::from(offset))
    .fetch_all(&state.pool)
    .await?;

    let (total,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM kyc_applications WHERE status = $1")
            .bind(status_filter)
            .fetch_one(&state.pool)
            .await?;

    Ok(Json(KycQueueResponse {
        applications: applications.into_iter().map(Into::into).collect(),
        total,
        page,
        limit,
    }))
}

/// Verify KYC application (admin)
pub async fn verify_kyc(
    State(state): State<AppState>,
    Extension(admin): Extension<crate::middleware::auth::AdminUser>,
    Path(kyc_id): Path<Uuid>,
    Json(payload): Json<VerifyKycRequest>,
) -> Result<Json<KycApplicationResponse>> {
    let admin_id = admin.0;
    let mut tx = state.pool.begin().await?;

    // Get application
    let application: KycApplication =
        sqlx::query_as("SELECT * FROM kyc_applications WHERE kyc_id = $1 FOR UPDATE")
            .bind(kyc_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| ApiError::NotFound("KYC application not found".to_string()))?;

    if application.status != "pending" && application.status != "in_review" {
        return Err(ApiError::Validation(format!(
            "Application is already {}",
            application.status
        )));
    }

    let (new_status, verified_at, expires_at) = if payload.approved {
        // Set expiration to 1 year from now
        let expires = Utc::now() + chrono::Duration::days(365);
        ("verified", Some(Utc::now()), Some(expires))
    } else {
        if payload.rejection_reason.is_none() {
            return Err(ApiError::Validation(
                "Rejection reason is required when rejecting".to_string(),
            ));
        }
        ("rejected", None, None)
    };

    let updated: KycApplication = sqlx::query_as(
        r#"
        UPDATE kyc_applications
        SET status = $2,
            rejection_reason = $3,
            reviewed_by = $4,
            reviewed_at = NOW(),
            verified_at = $5,
            expires_at = $6,
            updated_at = NOW()
        WHERE kyc_id = $1
        RETURNING *
        "#,
    )
    .bind(kyc_id)
    .bind(new_status)
    .bind(&payload.rejection_reason)
    .bind(admin_id)
    .bind(verified_at)
    .bind(expires_at)
    .fetch_one(&mut *tx)
    .await?;

    // Update user's KYC status
    if payload.approved {
        sqlx::query("UPDATE users SET kyc_status = 'verified', kyc_level = $2 WHERE user_id = $1")
            .bind(application.user_id)
            .bind(&updated.level)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;

    tracing::info!(
        kyc_id = %kyc_id,
        user_id = %application.user_id,
        admin_id = %admin_id,
        approved = payload.approved,
        "KYC application reviewed"
    );

    Ok(Json(updated.into()))
}

/// Get KYC statistics (admin)
pub async fn get_kyc_stats(State(state): State<AppState>) -> Result<Json<KycStats>> {
    let stats: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT status, COUNT(*) as count
        FROM kyc_applications
        GROUP BY status
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let mut result = KycStats::default();
    for (status, count) in stats {
        match status.as_str() {
            "pending" => result.pending = count,
            "in_review" => result.in_review = count,
            "verified" => result.verified = count,
            "rejected" => result.rejected = count,
            "expired" => result.expired = count,
            _ => {}
        }
    }
    result.total =
        result.pending + result.in_review + result.verified + result.rejected + result.expired;

    Ok(Json(result))
}

/// KYC statistics
#[derive(Debug, Clone, Serialize, Default)]
pub struct KycStats {
    /// Total number of KYC applications
    pub total: i64,
    /// Number of applications awaiting review
    pub pending: i64,
    /// Number of applications under manual review
    pub in_review: i64,
    /// Number of successfully verified applications
    pub verified: i64,
    /// Number of rejected applications
    pub rejected: i64,
    /// Number of expired verifications
    pub expired: i64,
}
