use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// User session model
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct UserSession {
    /// Unique session identifier.
    pub id: Uuid,
    /// User who owns this session.
    pub user_id: Uuid,
    /// Opaque session token used for authentication.
    pub session_token: String,
    /// Type of device (e.g., "mobile", "desktop").
    pub device_type: Option<String>,
    /// Human-readable device name.
    pub device_name: Option<String>,
    /// Operating system of the client device.
    pub os: Option<String>,
    /// Browser used to create the session.
    pub browser: Option<String>,
    /// IP address from which the session was created.
    pub ip_address: String,
    /// Country of the client IP address.
    pub country: Option<String>,
    /// City of the client IP address.
    pub city: Option<String>,
    /// Timestamp when the session was established.
    pub logged_in_at: DateTime<Utc>,
    /// Timestamp of the most recent activity in this session.
    pub last_activity_at: DateTime<Utc>,
    /// Timestamp when the session will expire.
    pub expires_at: DateTime<Utc>,
    /// Timestamp when the user explicitly logged out.
    pub logged_out_at: Option<DateTime<Utc>>,
    /// Whether the session is currently active.
    pub is_active: bool,
    /// Reason the session was terminated, if applicable.
    pub logout_reason: Option<String>,
    /// JSON flags for additional security context.
    pub security_flags: Option<JsonValue>,
    /// Record creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Record last-updated timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Parameters for creating a new session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSession {
    /// User to create the session for.
    pub user_id: Uuid,
    /// Unique session token.
    pub session_token: String,
    /// Device type (e.g., "mobile").
    pub device_type: Option<String>,
    /// Device name.
    pub device_name: Option<String>,
    /// Operating system.
    pub os: Option<String>,
    /// Browser name.
    pub browser: Option<String>,
    /// Client IP address.
    pub ip_address: String,
    /// Country derived from IP.
    pub country: Option<String>,
    /// City derived from IP.
    pub city: Option<String>,
    /// Session expiry timestamp.
    pub expires_at: DateTime<Utc>,
    /// Optional security metadata.
    pub security_flags: Option<JsonValue>,
}

/// Session statistics for a user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStats {
    /// Total number of sessions (all time).
    pub total_sessions: i64,
    /// Number of currently active sessions.
    pub active_sessions: i64,
    /// Number of distinct device types seen.
    pub unique_devices: i64,
    /// Number of distinct IP addresses seen.
    pub unique_ips: i64,
    /// Timestamp of the most recent login.
    pub last_login: Option<DateTime<Utc>>,
}

/// Active session summary
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct ActiveSessionSummary {
    /// Session identifier.
    pub id: Uuid,
    /// Device type.
    pub device_type: Option<String>,
    /// Device name.
    pub device_name: Option<String>,
    /// Client IP address.
    pub ip_address: String,
    /// Login timestamp.
    pub logged_in_at: DateTime<Utc>,
    /// Most recent activity timestamp.
    pub last_activity_at: DateTime<Utc>,
    /// Expiry timestamp.
    pub expires_at: DateTime<Utc>,
}

/// Session repository
pub struct SessionRepository {
    pool: PgPool,
}

impl SessionRepository {
    /// Create a new session repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new session
    pub async fn create(&self, params: CreateSession) -> Result<UserSession> {
        let session = sqlx::query_as::<_, UserSession>(
            r#"
            INSERT INTO user_sessions (
                user_id, session_token, device_type, device_name, os, browser,
                ip_address, country, city, expires_at, security_flags
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7::inet, $8, $9, $10, $11)
            RETURNING *
            "#,
        )
        .bind(params.user_id)
        .bind(&params.session_token)
        .bind(&params.device_type)
        .bind(&params.device_name)
        .bind(&params.os)
        .bind(&params.browser)
        .bind(&params.ip_address)
        .bind(&params.country)
        .bind(&params.city)
        .bind(params.expires_at)
        .bind(&params.security_flags)
        .fetch_one(&self.pool)
        .await?;

        Ok(session)
    }

    /// Find session by ID
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<UserSession>> {
        let session = sqlx::query_as::<_, UserSession>("SELECT * FROM user_sessions WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(session)
    }

    /// Find active session by token
    pub async fn find_by_token(&self, token: &str) -> Result<Option<UserSession>> {
        let session = sqlx::query_as::<_, UserSession>(
            r#"
            SELECT * FROM user_sessions
            WHERE session_token = $1
                AND is_active = true
                AND logged_out_at IS NULL
                AND expires_at > NOW()
            "#,
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await?;

        Ok(session)
    }

    /// Get all active sessions for a user
    pub async fn get_active_sessions(&self, user_id: Uuid) -> Result<Vec<UserSession>> {
        let sessions = sqlx::query_as::<_, UserSession>(
            r#"
            SELECT * FROM user_sessions
            WHERE user_id = $1
                AND is_active = true
                AND logged_out_at IS NULL
            ORDER BY last_activity_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(sessions)
    }

    /// Get active session summaries for a user (lightweight)
    pub async fn get_active_session_summaries(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<ActiveSessionSummary>> {
        let summaries = sqlx::query_as::<_, ActiveSessionSummary>(
            r#"
            SELECT id, device_type, device_name, ip_address, logged_in_at, last_activity_at, expires_at
            FROM user_sessions
            WHERE user_id = $1
                AND is_active = true
                AND logged_out_at IS NULL
            ORDER BY last_activity_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(summaries)
    }

    /// Get session history for a user
    pub async fn get_session_history(
        &self,
        user_id: Uuid,
        limit: i64,
        page: i64,
    ) -> Result<Vec<UserSession>> {
        let offset = crate::helpers::calculate_offset(page as u32, limit as u32);

        let sessions = sqlx::query_as::<_, UserSession>(
            r#"
            SELECT * FROM user_sessions
            WHERE user_id = $1
            ORDER BY logged_in_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(sessions)
    }

    /// Update last activity timestamp
    pub async fn update_activity(&self, session_id: Uuid) -> Result<UserSession> {
        let session = sqlx::query_as::<_, UserSession>(
            r#"
            UPDATE user_sessions
            SET last_activity_at = NOW(), updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(session)
    }

    /// Logout a session (mark as inactive)
    pub async fn logout(&self, session_id: Uuid, reason: &str) -> Result<UserSession> {
        let session = sqlx::query_as::<_, UserSession>(
            r#"
            UPDATE user_sessions
            SET is_active = false,
                logged_out_at = NOW(),
                logout_reason = $2,
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(session_id)
        .bind(reason)
        .fetch_one(&self.pool)
        .await?;

        Ok(session)
    }

    /// Logout all sessions for a user
    pub async fn logout_all_sessions(&self, user_id: Uuid, reason: &str) -> Result<i64> {
        let result = sqlx::query(
            r#"
            UPDATE user_sessions
            SET is_active = false,
                logged_out_at = NOW(),
                logout_reason = $2,
                updated_at = NOW()
            WHERE user_id = $1
                AND is_active = true
                AND logged_out_at IS NULL
            "#,
        )
        .bind(user_id)
        .bind(reason)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() as i64)
    }

    /// Logout all sessions except current one
    pub async fn logout_other_sessions(
        &self,
        user_id: Uuid,
        current_session_id: Uuid,
        reason: &str,
    ) -> Result<i64> {
        let result = sqlx::query(
            r#"
            UPDATE user_sessions
            SET is_active = false,
                logged_out_at = NOW(),
                logout_reason = $3,
                updated_at = NOW()
            WHERE user_id = $1
                AND id != $2
                AND is_active = true
                AND logged_out_at IS NULL
            "#,
        )
        .bind(user_id)
        .bind(current_session_id)
        .bind(reason)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() as i64)
    }

    /// Expire inactive sessions (cleanup)
    pub async fn expire_old_sessions(&self) -> Result<i64> {
        let result = sqlx::query(
            r#"
            UPDATE user_sessions
            SET is_active = false,
                logged_out_at = NOW(),
                logout_reason = 'expired',
                updated_at = NOW()
            WHERE is_active = true
                AND logged_out_at IS NULL
                AND expires_at < NOW()
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() as i64)
    }

    /// Delete old session records (GDPR compliance)
    pub async fn cleanup_old_sessions(&self, days: i64) -> Result<i64> {
        let result = sqlx::query(
            r#"
            DELETE FROM user_sessions
            WHERE is_active = false
                AND logged_out_at < NOW() - ($1 || ' days')::INTERVAL
            "#,
        )
        .bind(days)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() as i64)
    }

    /// Get session statistics for a user
    pub async fn get_user_stats(&self, user_id: Uuid) -> Result<SessionStats> {
        let row = sqlx::query(
            r#"
            SELECT
                COUNT(*) as total_sessions,
                COUNT(*) FILTER (WHERE is_active = true AND logged_out_at IS NULL) as active_sessions,
                COUNT(DISTINCT device_type) as unique_devices,
                COUNT(DISTINCT ip_address) as unique_ips,
                MAX(logged_in_at) as last_login
            FROM user_sessions
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(SessionStats {
            total_sessions: row.get("total_sessions"),
            active_sessions: row.get("active_sessions"),
            unique_devices: row.get("unique_devices"),
            unique_ips: row.get("unique_ips"),
            last_login: row.get("last_login"),
        })
    }

    /// Count active sessions for a user
    pub async fn count_active_sessions(&self, user_id: Uuid) -> Result<i64> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM user_sessions
            WHERE user_id = $1
                AND is_active = true
                AND logged_out_at IS NULL
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get("count"))
    }

    /// Get sessions by IP address (security analysis)
    pub async fn get_sessions_by_ip(
        &self,
        ip_address: &str,
        limit: i64,
    ) -> Result<Vec<UserSession>> {
        let sessions = sqlx::query_as::<_, UserSession>(
            r#"
            SELECT * FROM user_sessions
            WHERE ip_address = $1::inet
            ORDER BY logged_in_at DESC
            LIMIT $2
            "#,
        )
        .bind(ip_address)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(sessions)
    }

    /// Get sessions by device type
    pub async fn get_sessions_by_device(
        &self,
        user_id: Uuid,
        device_type: &str,
    ) -> Result<Vec<UserSession>> {
        let sessions = sqlx::query_as::<_, UserSession>(
            r#"
            SELECT * FROM user_sessions
            WHERE user_id = $1
                AND device_type = $2
            ORDER BY logged_in_at DESC
            "#,
        )
        .bind(user_id)
        .bind(device_type)
        .fetch_all(&self.pool)
        .await?;

        Ok(sessions)
    }

    /// Detect concurrent logins from different locations (security alert)
    pub async fn detect_suspicious_logins(
        &self,
        user_id: Uuid,
        time_window_minutes: i64,
    ) -> Result<bool> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(DISTINCT ip_address) as distinct_ips
            FROM user_sessions
            WHERE user_id = $1
                AND logged_in_at > NOW() - ($2 || ' minutes')::INTERVAL
            "#,
        )
        .bind(user_id)
        .bind(time_window_minutes)
        .fetch_one(&self.pool)
        .await?;

        let distinct_ips: i64 = row.get("distinct_ips");
        Ok(distinct_ips > 1) // Suspicious if logged in from multiple IPs in short time
    }

    /// Revoke session (security action)
    pub async fn revoke_session(&self, session_id: Uuid) -> Result<UserSession> {
        self.logout(session_id, "security").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_create_session_params() {
        let params = CreateSession {
            user_id: Uuid::new_v4(),
            session_token: "token123".to_string(),
            device_type: Some("desktop".to_string()),
            device_name: Some("Chrome on Windows".to_string()),
            os: Some("Windows 11".to_string()),
            browser: Some("Chrome 120".to_string()),
            ip_address: "192.168.1.1".to_string(),
            country: Some("US".to_string()),
            city: Some("San Francisco".to_string()),
            expires_at: Utc::now() + chrono::Duration::hours(24),
            security_flags: Some(json!({"2fa_verified": true})),
        };

        assert_eq!(params.device_type, Some("desktop".to_string()));
        assert_eq!(params.country, Some("US".to_string()));
    }

    #[test]
    fn test_session_stats_structure() {
        let stats = SessionStats {
            total_sessions: 10,
            active_sessions: 2,
            unique_devices: 3,
            unique_ips: 5,
            last_login: Some(Utc::now()),
        };

        assert_eq!(stats.total_sessions, 10);
        assert_eq!(stats.active_sessions, 2);
    }

    #[test]
    fn test_session_serialization() {
        let stats = SessionStats {
            total_sessions: 5,
            active_sessions: 1,
            unique_devices: 2,
            unique_ips: 3,
            last_login: None,
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("total_sessions"));

        let deserialized: SessionStats = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.total_sessions, 5);
    }

    #[test]
    fn test_ip_address_format() {
        let ip_str = "192.168.1.1";
        assert!(ip_str.contains('.'));
        // IP addresses are stored as strings in the database
    }
}
