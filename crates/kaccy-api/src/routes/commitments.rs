//! Commitment routes

use axum::{
    Json,
    extract::{Extension, Path, State},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::middleware::metrics::COMMITMENTS_CREATED_TOTAL;
use crate::state::AppState;

/// Request to create a new output commitment
#[derive(Debug, Deserialize)]
pub struct CreateCommitmentRequest {
    /// ID of the token this commitment is associated with
    pub token_id: Uuid,
    /// Commitment title (max 200 characters)
    pub title: String,
    /// Detailed description (max 2000 characters)
    pub description: Option<String>,
    /// Commitment deadline (must be in the future)
    pub deadline: DateTime<Utc>,
}

/// Request to submit evidence for a commitment
#[derive(Debug, Deserialize)]
pub struct SubmitEvidenceRequest {
    /// HTTPS URL to the evidence
    pub evidence_url: String,
    /// Optional description of the evidence
    pub evidence_description: Option<String>,
}

/// Commitment API response
#[derive(Debug, Serialize)]
pub struct CommitmentResponse {
    /// Unique commitment ID
    pub commitment_id: Uuid,
    /// ID of the user who created the commitment
    pub user_id: Uuid,
    /// ID of the associated token
    pub token_id: Uuid,
    /// Commitment title
    pub title: String,
    /// Detailed description
    pub description: Option<String>,
    /// Commitment deadline
    pub deadline: DateTime<Utc>,
    /// Current status (pending, completed, verified, failed)
    pub status: String,
    /// HTTPS URL to submitted evidence
    pub evidence_url: Option<String>,
    /// Description of the submitted evidence
    pub evidence_description: Option<String>,
    /// Timestamp when commitment was verified
    pub verified_at: Option<DateTime<Utc>>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct CommitmentRow {
    commitment_id: Uuid,
    user_id: Uuid,
    token_id: Uuid,
    title: String,
    description: Option<String>,
    deadline: DateTime<Utc>,
    status: String,
    evidence_url: Option<String>,
    evidence_description: Option<String>,
    verified_at: Option<DateTime<Utc>>,
    #[allow(dead_code)]
    verified_by: Option<Uuid>,
    created_at: DateTime<Utc>,
}

impl From<CommitmentRow> for CommitmentResponse {
    fn from(row: CommitmentRow) -> Self {
        Self {
            commitment_id: row.commitment_id,
            user_id: row.user_id,
            token_id: row.token_id,
            title: row.title,
            description: row.description,
            deadline: row.deadline,
            status: row.status,
            evidence_url: row.evidence_url,
            evidence_description: row.evidence_description,
            verified_at: row.verified_at,
            created_at: row.created_at,
        }
    }
}

/// Create a new commitment for a token
pub async fn create_commitment(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Json(req): Json<CreateCommitmentRequest>,
) -> Result<Json<CommitmentResponse>, ApiError> {
    // Validate request
    if req.title.is_empty() {
        return Err(ApiError::Validation("Title is required".to_string()));
    }
    if req.title.len() > 200 {
        return Err(ApiError::Validation(
            "Title must be at most 200 characters".to_string(),
        ));
    }
    if let Some(ref desc) = req.description {
        if desc.len() > 2000 {
            return Err(ApiError::Validation(
                "Description must be at most 2000 characters".to_string(),
            ));
        }
    }
    if req.deadline < Utc::now() {
        return Err(ApiError::Validation(
            "Deadline must be in the future".to_string(),
        ));
    }

    // Verify user owns this token
    let token_owner: Option<Uuid> =
        sqlx::query_scalar("SELECT issuer_user_id FROM tokens WHERE token_id = $1")
            .bind(req.token_id)
            .fetch_optional(&state.pool)
            .await?;

    match token_owner {
        Some(owner_id) if owner_id == user_id => {}
        Some(_) => {
            return Err(ApiError::Forbidden(
                "Only token issuer can create commitments".to_string(),
            ));
        }
        None => return Err(ApiError::NotFound("Token not found".to_string())),
    }

    let commitment = sqlx::query_as::<_, CommitmentRow>(
        r#"
        INSERT INTO output_commitments (user_id, token_id, title, description, deadline)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *
        "#,
    )
    .bind(user_id)
    .bind(req.token_id)
    .bind(&req.title)
    .bind(&req.description)
    .bind(req.deadline)
    .fetch_one(&state.pool)
    .await?;

    // Update metrics
    COMMITMENTS_CREATED_TOTAL.inc();

    Ok(Json(commitment.into()))
}

/// Get user's commitments
pub async fn get_my_commitments(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<Vec<CommitmentResponse>>, ApiError> {
    let commitments = sqlx::query_as::<_, CommitmentRow>(
        r#"
        SELECT * FROM output_commitments
        WHERE user_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(commitments.into_iter().map(Into::into).collect()))
}

/// Get token's commitments (public)
pub async fn get_token_commitments(
    State(state): State<AppState>,
    Path(token_id): Path<Uuid>,
) -> Result<Json<Vec<CommitmentResponse>>, ApiError> {
    let commitments = sqlx::query_as::<_, CommitmentRow>(
        r#"
        SELECT * FROM output_commitments
        WHERE token_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(token_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(commitments.into_iter().map(Into::into).collect()))
}

/// Submit evidence for a commitment
pub async fn submit_evidence(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Path(commitment_id): Path<Uuid>,
    Json(req): Json<SubmitEvidenceRequest>,
) -> Result<Json<CommitmentResponse>, ApiError> {
    // Validate
    if req.evidence_url.is_empty() {
        return Err(ApiError::Validation("Evidence URL is required".to_string()));
    }
    if !req.evidence_url.starts_with("https://") {
        return Err(ApiError::Validation(
            "Evidence URL must be HTTPS".to_string(),
        ));
    }

    let commitment = sqlx::query_as::<_, CommitmentRow>(
        r#"
        UPDATE output_commitments
        SET evidence_url = $3, evidence_description = $4, status = 'completed'
        WHERE commitment_id = $1 AND user_id = $2 AND status = 'pending'
        RETURNING *
        "#,
    )
    .bind(commitment_id)
    .bind(user_id)
    .bind(&req.evidence_url)
    .bind(&req.evidence_description)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Commitment not found or already submitted".to_string()))?;

    Ok(Json(commitment.into()))
}

/// Get a single commitment
pub async fn get_commitment(
    State(state): State<AppState>,
    Path(commitment_id): Path<Uuid>,
) -> Result<Json<CommitmentResponse>, ApiError> {
    let commitment = sqlx::query_as::<_, CommitmentRow>(
        "SELECT * FROM output_commitments WHERE commitment_id = $1",
    )
    .bind(commitment_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Commitment not found".to_string()))?;

    Ok(Json(commitment.into()))
}
