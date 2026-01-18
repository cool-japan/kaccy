//! Admin routes

use axum::{
    Json,
    extract::{Extension, Path, Query, State},
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::middleware::{AdminUser, metrics::COMMITMENTS_VERIFIED_TOTAL};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct PendingCommitmentsQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct PendingCommitment {
    pub commitment_id: Uuid,
    pub user_id: Uuid,
    pub token_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub deadline: DateTime<Utc>,
    pub evidence_url: Option<String>,
    pub evidence_description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub user_username: String,
    pub user_reputation: Decimal,
    pub token_symbol: String,
}

#[derive(Debug, sqlx::FromRow)]
struct PendingCommitmentRow {
    commitment_id: Uuid,
    user_id: Uuid,
    token_id: Uuid,
    title: String,
    description: Option<String>,
    deadline: DateTime<Utc>,
    evidence_url: Option<String>,
    evidence_description: Option<String>,
    created_at: DateTime<Utc>,
    user_username: String,
    user_reputation: Decimal,
    token_symbol: String,
}

impl From<PendingCommitmentRow> for PendingCommitment {
    fn from(row: PendingCommitmentRow) -> Self {
        Self {
            commitment_id: row.commitment_id,
            user_id: row.user_id,
            token_id: row.token_id,
            title: row.title,
            description: row.description,
            deadline: row.deadline,
            evidence_url: row.evidence_url,
            evidence_description: row.evidence_description,
            created_at: row.created_at,
            user_username: row.user_username,
            user_reputation: row.user_reputation,
            token_symbol: row.token_symbol,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct VerifyCommitmentRequest {
    pub fulfilled: bool,
    pub quality_score: Option<Decimal>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct VerifyCommitmentResponse {
    pub commitment_id: Uuid,
    pub status: String,
    pub reputation_delta: Decimal,
    pub new_user_reputation: Decimal,
}

/// Get pending commitments for verification
pub async fn get_pending_commitments(
    State(state): State<AppState>,
    Extension(_admin): Extension<AdminUser>,
    Query(query): Query<PendingCommitmentsQuery>,
) -> Result<Json<Vec<PendingCommitment>>, ApiError> {
    let limit = query.limit.unwrap_or(50).min(100);

    let commitments = sqlx::query_as::<_, PendingCommitmentRow>(
        r#"
        SELECT
            c.commitment_id,
            c.user_id,
            c.token_id,
            c.title,
            c.description,
            c.deadline,
            c.evidence_url,
            c.evidence_description,
            c.created_at,
            u.username as user_username,
            u.reputation_score as user_reputation,
            t.symbol as token_symbol
        FROM output_commitments c
        JOIN users u ON c.user_id = u.user_id
        JOIN tokens t ON c.token_id = t.token_id
        WHERE c.status = 'completed'
        ORDER BY c.created_at ASC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(commitments.into_iter().map(Into::into).collect()))
}

/// Verify a commitment (approve or reject)
pub async fn verify_commitment(
    State(state): State<AppState>,
    Extension(admin): Extension<AdminUser>,
    Path(commitment_id): Path<Uuid>,
    Json(req): Json<VerifyCommitmentRequest>,
) -> Result<Json<VerifyCommitmentResponse>, ApiError> {
    // Validate quality score if provided
    if let Some(score) = req.quality_score {
        if score < Decimal::ZERO || score > Decimal::from(100) {
            return Err(ApiError::Validation(
                "Quality score must be between 0 and 100".to_string(),
            ));
        }
    }

    let mut tx = state.pool.begin().await?;

    // Get the commitment
    let commitment: Option<(Uuid, String)> = sqlx::query_as(
        r#"
        SELECT user_id, status FROM output_commitments
        WHERE commitment_id = $1
        FOR UPDATE
        "#,
    )
    .bind(commitment_id)
    .fetch_optional(&mut *tx)
    .await?;

    let (user_id, status) =
        commitment.ok_or_else(|| ApiError::NotFound("Commitment not found".to_string()))?;

    if status != "completed" {
        return Err(ApiError::Validation(format!(
            "Commitment status is '{}', expected 'completed'",
            status
        )));
    }

    let new_status = if req.fulfilled { "verified" } else { "failed" };

    // Update commitment
    sqlx::query(
        r#"
        UPDATE output_commitments
        SET status = $2, verified_at = NOW(), verified_by = $3
        WHERE commitment_id = $1
        "#,
    )
    .bind(commitment_id)
    .bind(new_status)
    .bind(admin.0)
    .execute(&mut *tx)
    .await?;

    // Calculate reputation delta
    let base_delta = if req.fulfilled {
        Decimal::from(10) // +10 for fulfillment
    } else {
        Decimal::from(-20) // -20 for failure (2x penalty)
    };

    // Add quality bonus if fulfilled
    let quality_bonus = if req.fulfilled {
        req.quality_score
            .map(|q| (q - Decimal::from(50)) / Decimal::from(10))
            .unwrap_or(Decimal::ZERO)
    } else {
        Decimal::ZERO
    };

    let total_delta = base_delta + quality_bonus;

    // Update user reputation
    let new_reputation: Decimal = sqlx::query_scalar(
        r#"
        UPDATE users
        SET reputation_score = GREATEST(0, LEAST(1000, reputation_score + $2))
        WHERE user_id = $1
        RETURNING reputation_score
        "#,
    )
    .bind(user_id)
    .bind(total_delta)
    .fetch_one(&mut *tx)
    .await?;

    // Record reputation event
    sqlx::query(
        r#"
        INSERT INTO reputation_events (user_id, event_type, delta, reason)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(user_id)
    .bind(if req.fulfilled {
        "commitment_fulfilled"
    } else {
        "commitment_failed"
    })
    .bind(total_delta)
    .bind(&req.notes)
    .execute(&mut *tx)
    .await?;

    // Record admin action
    sqlx::query(
        r#"
        INSERT INTO admin_actions (admin_user_id, action_type, target_type, target_id, details)
        VALUES ($1, $2, 'commitment', $3, $4)
        "#,
    )
    .bind(admin.0)
    .bind(if req.fulfilled {
        "verify_commitment"
    } else {
        "reject_commitment"
    })
    .bind(commitment_id)
    .bind(serde_json::json!({
        "quality_score": req.quality_score,
        "notes": req.notes,
        "reputation_delta": total_delta,
    }))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    // Update metrics
    let status_label = if req.fulfilled {
        "approved"
    } else {
        "rejected"
    };
    COMMITMENTS_VERIFIED_TOTAL
        .with_label_values(&[status_label])
        .inc();

    Ok(Json(VerifyCommitmentResponse {
        commitment_id,
        status: new_status.to_string(),
        reputation_delta: total_delta,
        new_user_reputation: new_reputation,
    }))
}

#[derive(Debug, Serialize)]
pub struct AdminStats {
    pub total_users: i64,
    pub total_tokens: i64,
    pub total_trades: i64,
    pub pending_commitments: i64,
    pub total_volume_btc: Option<Decimal>,
}

/// Get admin dashboard stats
pub async fn get_admin_stats(
    State(state): State<AppState>,
    Extension(_admin): Extension<AdminUser>,
) -> Result<Json<AdminStats>, ApiError> {
    let total_users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.pool)
        .await?;

    let total_tokens: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tokens")
        .fetch_one(&state.pool)
        .await?;

    let total_trades: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM trades")
        .fetch_one(&state.pool)
        .await?;

    let pending_commitments: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM output_commitments WHERE status = 'completed'")
            .fetch_one(&state.pool)
            .await?;

    let total_volume_btc: Option<Decimal> = sqlx::query_scalar("SELECT SUM(total_btc) FROM trades")
        .fetch_one(&state.pool)
        .await?;

    Ok(Json(AdminStats {
        total_users,
        total_tokens,
        total_trades,
        pending_commitments,
        total_volume_btc,
    }))
}

#[derive(Debug, Deserialize)]
pub struct UsersQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub role: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AdminUserView {
    pub user_id: Uuid,
    pub email: String,
    pub username: String,
    pub display_name: Option<String>,
    pub kyc_status: String,
    pub role: String,
    pub reputation_score: Decimal,
    pub created_at: DateTime<Utc>,
}

/// List users (admin)
pub async fn list_users(
    State(state): State<AppState>,
    Extension(_admin): Extension<AdminUser>,
    Query(query): Query<UsersQuery>,
) -> Result<Json<Vec<AdminUserView>>, ApiError> {
    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

    let users = if let Some(role) = query.role {
        sqlx::query_as::<_, AdminUserView>(
            r#"
            SELECT user_id, email, username, display_name, kyc_status, role, reputation_score, created_at
            FROM users
            WHERE role = $3
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .bind(role)
        .fetch_all(&state.pool)
        .await?
    } else {
        sqlx::query_as::<_, AdminUserView>(
            r#"
            SELECT user_id, email, username, display_name, kyc_status, role, reputation_score, created_at
            FROM users
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await?
    };

    Ok(Json(users))
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRoleRequest {
    pub role: String,
}

/// Update user role (admin)
pub async fn update_user_role(
    State(state): State<AppState>,
    Extension(admin): Extension<AdminUser>,
    Path(user_id): Path<Uuid>,
    Json(req): Json<UpdateUserRoleRequest>,
) -> Result<Json<AdminUserView>, ApiError> {
    // Validate role
    if !["user", "admin"].contains(&req.role.as_str()) {
        return Err(ApiError::Validation(
            "Role must be 'user' or 'admin'".to_string(),
        ));
    }

    // Prevent self-demotion
    if user_id == admin.0 && req.role == "user" {
        return Err(ApiError::Validation("Cannot demote yourself".to_string()));
    }

    let mut tx = state.pool.begin().await?;

    let user = sqlx::query_as::<_, AdminUserView>(
        r#"
        UPDATE users SET role = $2
        WHERE user_id = $1
        RETURNING user_id, email, username, display_name, kyc_status, role, reputation_score, created_at
        "#,
    )
    .bind(user_id)
    .bind(&req.role)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::NotFound("User not found".to_string()))?;

    // Record admin action
    sqlx::query(
        r#"
        INSERT INTO admin_actions (admin_user_id, action_type, target_type, target_id, details)
        VALUES ($1, 'update_role', 'user', $2, $3)
        "#,
    )
    .bind(admin.0)
    .bind(user_id)
    .bind(serde_json::json!({ "new_role": req.role }))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(user))
}
