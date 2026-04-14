//! User session management routes
//!
//! Provides endpoints for managing user sessions

use axum::{
    Extension, Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{error::ApiError, middleware::Claims, state::AppState};
use kaccy_db::repositories::session::{
    ActiveSessionSummary, SessionRepository, SessionStats, UserSession,
};

/// Response for active sessions list
#[derive(Debug, Serialize)]
pub struct ActiveSessionsResponse {
    /// List of currently active sessions
    pub sessions: Vec<ActiveSessionSummary>,
    /// Aggregate session statistics
    pub stats: SessionStats,
}

/// Response for session history
#[derive(Debug, Serialize)]
pub struct SessionHistoryResponse {
    /// Historical sessions for the user
    pub sessions: Vec<UserSession>,
    /// Total number of historical session records
    pub total: usize,
}

/// Logout request
#[derive(Debug, Serialize, Deserialize)]
pub struct LogoutRequest {
    /// Optional reason for logout
    pub reason: Option<String>,
}

/// Get current user's active sessions
///
/// Protected endpoint - users can view their own sessions
#[allow(dead_code)]
pub async fn get_my_sessions(
    Extension(user_id): Extension<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<ActiveSessionsResponse>, ApiError> {
    let repo = SessionRepository::new(state.pool.clone());

    let sessions = repo.get_active_session_summaries(user_id).await?;

    let stats = repo.get_user_stats(user_id).await?;

    Ok(Json(ActiveSessionsResponse { sessions, stats }))
}

/// Get session history for current user
///
/// Protected endpoint - users can view their own session history
#[allow(dead_code)]
pub async fn get_my_session_history(
    Extension(user_id): Extension<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<SessionHistoryResponse>, ApiError> {
    let repo = SessionRepository::new(state.pool.clone());

    let sessions = repo
        .get_session_history(user_id, 50, 0) // limit=50, page=0
        .await?;

    let total = sessions.len();

    Ok(Json(SessionHistoryResponse { sessions, total }))
}

/// Logout a specific session
///
/// Protected endpoint - users can logout their own sessions
#[allow(dead_code)]
pub async fn logout_session(
    Extension(user_id): Extension<Uuid>,
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(request): Json<LogoutRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let repo = SessionRepository::new(state.pool.clone());

    // Verify the session belongs to the user
    let session = repo
        .find_by_id(session_id)
        .await?
        .ok_or(ApiError::NotFound("Session not found".to_string()))?;

    if session.user_id != user_id {
        return Err(ApiError::Forbidden(
            "You can only logout your own sessions".to_string(),
        ));
    }

    let reason = request.reason.as_deref().unwrap_or("user_logout");
    repo.logout(session_id, reason).await?;

    Ok(Json(serde_json::json!({
        "message": "Session logged out successfully"
    })))
}

/// Logout all other sessions (keep current session)
///
/// Protected endpoint - users can logout all their other sessions
#[allow(dead_code)]
pub async fn logout_other_sessions(
    Extension(user_id): Extension<Uuid>,
    State(state): State<AppState>,
    Json(request): Json<LogoutOtherSessionsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let repo = SessionRepository::new(state.pool.clone());

    let reason = request.reason.as_deref().unwrap_or("logout_other_sessions");

    let logout_count = repo
        .logout_other_sessions(user_id, request.current_session_id, reason)
        .await?;

    Ok(Json(serde_json::json!({
        "message": format!("Logged out {} other sessions", logout_count),
        "logout_count": logout_count
    })))
}

/// Request for logging out other sessions
#[derive(Debug, Deserialize)]
pub struct LogoutOtherSessionsRequest {
    /// Current session ID to keep active
    pub current_session_id: Uuid,
    /// Optional reason for logout
    pub reason: Option<String>,
}

/// Logout all sessions for current user
///
/// Protected endpoint - users can logout all their sessions
#[allow(dead_code)]
pub async fn logout_all_sessions(
    Extension(user_id): Extension<Uuid>,
    State(state): State<AppState>,
    Json(request): Json<LogoutRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let repo = SessionRepository::new(state.pool.clone());

    let reason = request.reason.as_deref().unwrap_or("logout_all");

    let logout_count = repo.logout_all_sessions(user_id, reason).await?;

    Ok(Json(serde_json::json!({
        "message": format!("Logged out {} sessions", logout_count),
        "logout_count": logout_count
    })))
}

/// Get active session count for a specific user
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn get_active_session_count(
    _claims: Claims,
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let repo = SessionRepository::new(state.pool.clone());

    let count = repo.count_active_sessions(user_id).await?;

    Ok(Json(serde_json::json!({
        "user_id": user_id,
        "active_sessions": count
    })))
}

/// Check for suspicious login activity for a specific user
///
/// Admin only endpoint - checks if user has suspicious login pattern
#[allow(dead_code)]
pub async fn check_suspicious_logins(
    _claims: Claims,
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let repo = SessionRepository::new(state.pool.clone());

    let time_window_minutes = 60; // Look at last 60 minutes

    let is_suspicious = repo
        .detect_suspicious_logins(user_id, time_window_minutes)
        .await?;

    Ok(Json(serde_json::json!({
        "user_id": user_id,
        "is_suspicious": is_suspicious,
        "time_window_minutes": time_window_minutes
    })))
}

/// Revoke a session (admin only)
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn revoke_session_admin(
    _claims: Claims,
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(_request): Json<RevokeSessionRequest>,
) -> Result<Json<UserSession>, ApiError> {
    let repo = SessionRepository::new(state.pool.clone());

    let session = repo.revoke_session(session_id).await?;

    Ok(Json(session))
}

/// Request for revoking a session
#[derive(Debug, Deserialize)]
pub struct RevokeSessionRequest {
    /// Reason for revoking the session
    pub reason: String,
}

/// Get sessions by IP address (admin only)
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn get_sessions_by_ip(
    _claims: Claims,
    State(state): State<AppState>,
    Path(ip_address): Path<String>,
) -> Result<Json<Vec<UserSession>>, ApiError> {
    let repo = SessionRepository::new(state.pool.clone());

    let sessions = repo
        .get_sessions_by_ip(&ip_address, 100) // Default limit of 100 sessions
        .await?;

    Ok(Json(sessions))
}

/// Get sessions by device type for a specific user
///
/// Admin endpoint - requires user_id and device_type in path
#[allow(dead_code)]
pub async fn get_sessions_by_device(
    _claims: Claims,
    State(state): State<AppState>,
    Path((user_id, device_type)): Path<(Uuid, String)>,
) -> Result<Json<Vec<UserSession>>, ApiError> {
    let repo = SessionRepository::new(state.pool.clone());

    let sessions = repo.get_sessions_by_device(user_id, &device_type).await?;

    Ok(Json(sessions))
}

/// Cleanup expired sessions (admin only)
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn cleanup_expired_sessions(
    _claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let repo = SessionRepository::new(state.pool.clone());

    let deleted_count = repo.expire_old_sessions().await?;

    Ok(Json(serde_json::json!({
        "message": format!("Expired {} sessions", deleted_count),
        "deleted_count": deleted_count
    })))
}

/// Cleanup old session records (admin only)
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn cleanup_old_sessions(
    _claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let repo = SessionRepository::new(state.pool.clone());

    let days_old = 90; // Keep last 90 days

    let deleted_count = repo.cleanup_old_sessions(days_old).await?;

    Ok(Json(serde_json::json!({
        "message": format!("Deleted {} old sessions", deleted_count),
        "deleted_count": deleted_count
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logout_request_serialization() {
        let request = LogoutRequest {
            reason: Some("User initiated".to_string()),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("User initiated"));
    }

    #[test]
    fn test_revoke_session_request() {
        let request = RevokeSessionRequest {
            reason: "Security violation".to_string(),
        };
        assert_eq!(request.reason, "Security violation");
    }
}
