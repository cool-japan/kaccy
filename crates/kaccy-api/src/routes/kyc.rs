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
    pub fn from_db_string(s: &str) -> Self {
        match s {
            "none" => KycLevel::None,
            "basic" => KycLevel::Basic,
            "standard" => KycLevel::Standard,
            "enhanced" => KycLevel::Enhanced,
            _ => KycLevel::None,
        }
    }

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
    pub kyc_id: Uuid,
    pub user_id: Uuid,
    pub status: String,
    pub level: String,
    pub full_name: Option<String>,
    pub date_of_birth: Option<String>,
    pub country: Option<String>,
    pub document_type: Option<String>,
    pub document_number: Option<String>,
    pub document_expiry: Option<String>,
    pub selfie_url: Option<String>,
    pub document_front_url: Option<String>,
    pub document_back_url: Option<String>,
    pub proof_of_address_url: Option<String>,
    pub rejection_reason: Option<String>,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub verified_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// KYC application response (public-safe version)
#[derive(Debug, Clone, Serialize)]
pub struct KycApplicationResponse {
    pub kyc_id: Uuid,
    pub user_id: Uuid,
    pub status: KycStatus,
    pub level: KycLevel,
    pub full_name: Option<String>,
    pub country: Option<String>,
    pub rejection_reason: Option<String>,
    pub verified_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
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
    pub full_name: String,
    pub date_of_birth: String,
    pub country: String,
    pub document_type: String,
    pub document_number: String,
    pub document_expiry: Option<String>,
    pub selfie_url: String,
    pub document_front_url: String,
    pub document_back_url: Option<String>,
    pub proof_of_address_url: Option<String>,
}

impl SubmitKycRequest {
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
    pub approved: bool,
    pub rejection_reason: Option<String>,
}

/// KYC queue query params
#[derive(Debug, Clone, Deserialize)]
pub struct KycQueueQuery {
    pub status: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

/// KYC queue response
#[derive(Debug, Clone, Serialize)]
pub struct KycQueueResponse {
    pub applications: Vec<KycApplicationResponse>,
    pub total: i64,
    pub page: u32,
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
    pub total: i64,
    pub pending: i64,
    pub in_review: i64,
    pub verified: i64,
    pub rejected: i64,
    pub expired: i64,
}
