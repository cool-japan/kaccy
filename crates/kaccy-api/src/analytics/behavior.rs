use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// User action types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum UserAction {
    /// User viewed a page
    PageView {
        /// Page path viewed
        page: String,
        /// Referrer URL
        referrer: Option<String>,
    },
    /// User clicked a button
    ButtonClick {
        /// Button element ID
        button_id: String,
        /// Button label text
        label: String,
    },
    /// User submitted a form
    FormSubmit {
        /// Form element ID
        form_id: String,
        /// Whether the form submission was successful
        success: bool,
    },
    /// User purchased tokens
    TokenPurchase {
        /// Token that was purchased
        token_id: Uuid,
        /// Amount purchased as string
        amount: String,
    },
    /// User sold tokens
    TokenSale {
        /// Token that was sold
        token_id: Uuid,
        /// Amount sold as string
        amount: String,
    },
    /// User performed a search
    Search {
        /// Search query
        query: String,
        /// Number of results returned
        results_count: usize,
    },
    /// User updated their profile
    ProfileUpdate {
        /// List of fields that were updated
        fields: Vec<String>,
    },
}

/// User session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSession {
    /// Session ID
    pub session_id: String,
    /// User ID
    pub user_id: Uuid,
    /// Start time
    pub started_at: DateTime<Utc>,
    /// Last activity time
    pub last_activity: DateTime<Utc>,
    /// Actions in this session
    pub actions: Vec<(DateTime<Utc>, UserAction)>,
}

impl UserSession {
    /// Create a new user session
    pub fn new(session_id: String, user_id: Uuid) -> Self {
        let now = Utc::now();
        Self {
            session_id,
            user_id,
            started_at: now,
            last_activity: now,
            actions: Vec::new(),
        }
    }

    /// Add an action to the session
    pub fn add_action(&mut self, action: UserAction) {
        self.last_activity = Utc::now();
        self.actions.push((Utc::now(), action));
    }

    /// Get session duration
    pub fn duration(&self) -> Duration {
        self.last_activity - self.started_at
    }

    /// Check if session is expired
    pub fn is_expired(&self, timeout: Duration) -> bool {
        Utc::now() - self.last_activity > timeout
    }
}

/// Behavior tracker
pub struct BehaviorTracker {
    /// Active sessions by session ID
    sessions: Arc<DashMap<String, UserSession>>,
    /// Session timeout duration
    session_timeout: Duration,
}

impl BehaviorTracker {
    /// Create a new behavior tracker
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            session_timeout: Duration::minutes(30),
        }
    }

    /// Create with custom timeout
    pub fn with_timeout(session_timeout: Duration) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            session_timeout,
        }
    }

    /// Start or resume a session
    pub async fn start_session(&self, session_id: String, user_id: Uuid) {
        self.sessions
            .entry(session_id.clone())
            .or_insert_with(|| UserSession::new(session_id, user_id));
    }

    /// Track a user action
    pub async fn track_action(
        &self,
        session_id: &str,
        user_id: Uuid,
        action: UserAction,
    ) -> anyhow::Result<()> {
        // Ensure session exists
        self.start_session(session_id.to_string(), user_id).await;

        // Add action to session
        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.add_action(action);
            Ok(())
        } else {
            anyhow::bail!("Session not found")
        }
    }

    /// Get a session
    pub async fn get_session(&self, session_id: &str) -> Option<UserSession> {
        self.sessions.get(session_id).map(|s| s.value().clone())
    }

    /// Get user sessions
    pub async fn get_user_sessions(&self, user_id: &Uuid) -> Vec<UserSession> {
        self.sessions
            .iter()
            .filter(|s| &s.value().user_id == user_id)
            .map(|s| s.value().clone())
            .collect()
    }

    /// Clean up expired sessions
    pub async fn cleanup_expired_sessions(&self) -> usize {
        let expired: Vec<String> = self
            .sessions
            .iter()
            .filter(|s| s.value().is_expired(self.session_timeout))
            .map(|s| s.value().session_id.clone())
            .collect();

        let count = expired.len();
        for session_id in expired {
            self.sessions.remove(&session_id);
        }

        if count > 0 {
            tracing::info!("Cleaned up {} expired sessions", count);
        }

        count
    }

    /// Get session statistics for a user
    pub async fn get_user_stats(&self, user_id: &Uuid) -> UserStats {
        let sessions = self.get_user_sessions(user_id).await;

        let total_sessions = sessions.len();
        let total_actions: usize = sessions.iter().map(|s| s.actions.len()).sum();
        let avg_session_duration = if total_sessions > 0 {
            let total_duration: i64 = sessions.iter().map(|s| s.duration().num_seconds()).sum();
            Duration::seconds(total_duration / total_sessions as i64)
        } else {
            Duration::zero()
        };

        UserStats {
            total_sessions,
            total_actions,
            avg_session_duration,
        }
    }
}

impl Default for BehaviorTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// User statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStats {
    /// Total number of sessions
    pub total_sessions: usize,
    /// Total number of actions across all sessions
    pub total_actions: usize,
    /// Average session duration
    pub avg_session_duration: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_behavior_tracker_creation() {
        let tracker = BehaviorTracker::new();
        let sessions = tracker.get_user_sessions(&Uuid::new_v4()).await;
        assert_eq!(sessions.len(), 0);
    }

    #[tokio::test]
    async fn test_start_session() {
        let tracker = BehaviorTracker::new();
        let session_id = "test-session-123".to_string();
        let user_id = Uuid::new_v4();

        tracker.start_session(session_id.clone(), user_id).await;

        let session = tracker.get_session(&session_id).await;
        assert!(session.is_some());
        assert_eq!(session.unwrap().user_id, user_id);
    }

    #[tokio::test]
    async fn test_track_action() {
        let tracker = BehaviorTracker::new();
        let session_id = "test-session-123".to_string();
        let user_id = Uuid::new_v4();

        tracker.start_session(session_id.clone(), user_id).await;

        let action = UserAction::PageView {
            page: "/dashboard".to_string(),
            referrer: None,
        };

        tracker
            .track_action(&session_id, user_id, action)
            .await
            .unwrap();

        let session = tracker.get_session(&session_id).await.unwrap();
        assert_eq!(session.actions.len(), 1);
    }

    #[tokio::test]
    async fn test_user_stats() {
        let tracker = BehaviorTracker::new();
        let session_id = "test-session-123".to_string();
        let user_id = Uuid::new_v4();

        tracker.start_session(session_id.clone(), user_id).await;

        tracker
            .track_action(
                &session_id,
                user_id,
                UserAction::PageView {
                    page: "/dashboard".to_string(),
                    referrer: None,
                },
            )
            .await
            .unwrap();

        tracker
            .track_action(
                &session_id,
                user_id,
                UserAction::ButtonClick {
                    button_id: "btn-1".to_string(),
                    label: "Click me".to_string(),
                },
            )
            .await
            .unwrap();

        let stats = tracker.get_user_stats(&user_id).await;
        assert_eq!(stats.total_sessions, 1);
        assert_eq!(stats.total_actions, 2);
    }
}
