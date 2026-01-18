//! Cache invalidation patterns for distributed systems.
//!
//! This module provides:
//! - Redis pub/sub for cross-instance invalidation
//! - Tag-based invalidation
//! - Cascade invalidation rules

use futures::StreamExt;
use parking_lot::RwLock;
use redis::Client;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::cache::RedisCache;
use crate::error::{DbError, Result};

/// Invalidation event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidationEvent {
    /// Type of invalidation
    pub event_type: InvalidationType,
    /// Keys to invalidate
    pub keys: Vec<String>,
    /// Tags to invalidate
    pub tags: Vec<String>,
    /// Timestamp of the event
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Source instance that triggered the invalidation
    pub source_instance: String,
}

/// Type of invalidation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvalidationType {
    /// Invalidate specific keys
    Keys,
    /// Invalidate all keys with a tag
    Tags,
    /// Invalidate with cascade rules
    Cascade,
    /// Invalidate all keys matching a pattern
    Pattern,
}

/// Configuration for invalidation manager
#[derive(Debug, Clone)]
pub struct InvalidationConfig {
    /// Redis pub/sub channel name
    pub pubsub_channel: String,
    /// Instance identifier (for preventing self-invalidation loops)
    pub instance_id: String,
    /// Enable cascade invalidation
    pub enable_cascade: bool,
    /// Maximum cascade depth
    pub max_cascade_depth: usize,
}

impl Default for InvalidationConfig {
    fn default() -> Self {
        Self {
            pubsub_channel: "cache:invalidation".to_string(),
            instance_id: uuid::Uuid::new_v4().to_string(),
            enable_cascade: true,
            max_cascade_depth: 5,
        }
    }
}

/// Tag registry for managing key-tag relationships
#[derive(Debug, Clone)]
pub struct TagRegistry {
    /// Map of tags to keys
    tags: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    /// Map of keys to tags
    keys: Arc<RwLock<HashMap<String, HashSet<String>>>>,
}

impl Default for TagRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TagRegistry {
    /// Create a new tag registry
    pub fn new() -> Self {
        Self {
            tags: Arc::new(RwLock::new(HashMap::new())),
            keys: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a key with tags
    pub fn register(&self, key: String, tags: Vec<String>) {
        let mut tag_map = self.tags.write();
        let mut key_map = self.keys.write();

        for tag in &tags {
            tag_map.entry(tag.clone()).or_default().insert(key.clone());
        }

        key_map.insert(key, tags.into_iter().collect());
    }

    /// Get all keys for a tag
    pub fn get_keys_for_tag(&self, tag: &str) -> Vec<String> {
        self.tags
            .read()
            .get(tag)
            .map(|keys| keys.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get all tags for a key
    pub fn get_tags_for_key(&self, key: &str) -> Vec<String> {
        self.keys
            .read()
            .get(key)
            .map(|tags| tags.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Unregister a key
    pub fn unregister(&self, key: &str) {
        let mut key_map = self.keys.write();
        if let Some(tags) = key_map.remove(key) {
            let mut tag_map = self.tags.write();
            for tag in tags {
                if let Some(keys) = tag_map.get_mut(&tag) {
                    keys.remove(key);
                }
            }
        }
    }
}

/// Cascade rule for invalidation
#[derive(Debug, Clone)]
pub struct CascadeRule {
    /// Source tag that triggers the cascade
    pub source_tag: String,
    /// Target tags to invalidate
    pub target_tags: Vec<String>,
}

/// Invalidation manager
pub struct InvalidationManager {
    cache: Arc<RedisCache>,
    config: InvalidationConfig,
    tag_registry: TagRegistry,
    cascade_rules: Arc<RwLock<Vec<CascadeRule>>>,
    pubsub_tx: mpsc::UnboundedSender<InvalidationEvent>,
}

impl InvalidationManager {
    /// Create a new invalidation manager
    pub fn new(
        cache: Arc<RedisCache>,
        config: InvalidationConfig,
    ) -> (Self, mpsc::UnboundedReceiver<InvalidationEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();

        let manager = Self {
            cache,
            config,
            tag_registry: TagRegistry::new(),
            cascade_rules: Arc::new(RwLock::new(Vec::new())),
            pubsub_tx: tx,
        };

        (manager, rx)
    }

    /// Register a cascade rule
    pub fn add_cascade_rule(&self, rule: CascadeRule) {
        info!(
            source = %rule.source_tag,
            targets = ?rule.target_tags,
            "Added cascade invalidation rule"
        );
        self.cascade_rules.write().push(rule);
    }

    /// Register a key with tags
    pub fn register_key(&self, key: String, tags: Vec<String>) {
        self.tag_registry.register(key, tags);
    }

    /// Invalidate specific keys
    pub async fn invalidate_keys(&self, keys: Vec<String>) -> Result<()> {
        debug!(count = keys.len(), "Invalidating keys");

        for key in &keys {
            if let Err(e) = self.cache.delete(key).await {
                error!(key = %key, error = %e, "Failed to invalidate key");
            }
        }

        // Publish invalidation event
        let event = InvalidationEvent {
            event_type: InvalidationType::Keys,
            keys,
            tags: Vec::new(),
            timestamp: chrono::Utc::now(),
            source_instance: self.config.instance_id.clone(),
        };

        self.publish_event(event).await?;

        Ok(())
    }

    /// Invalidate all keys with a specific tag
    pub async fn invalidate_tag(&self, tag: String) -> Result<()> {
        let keys = self.tag_registry.get_keys_for_tag(&tag);

        debug!(tag = %tag, key_count = keys.len(), "Invalidating tag");

        for key in &keys {
            if let Err(e) = self.cache.delete(key).await {
                error!(key = %key, error = %e, "Failed to invalidate key");
            }
        }

        // Apply cascade rules if enabled
        if self.config.enable_cascade {
            self.apply_cascade_rules(&tag, 0).await?;
        }

        // Publish invalidation event
        let event = InvalidationEvent {
            event_type: InvalidationType::Tags,
            keys: Vec::new(),
            tags: vec![tag],
            timestamp: chrono::Utc::now(),
            source_instance: self.config.instance_id.clone(),
        };

        self.publish_event(event).await?;

        Ok(())
    }

    /// Invalidate keys matching a pattern
    pub async fn invalidate_pattern(&self, pattern: String) -> Result<()> {
        debug!(pattern = %pattern, "Invalidating pattern");

        let deleted = self.cache.delete_pattern(&pattern).await?;

        info!(pattern = %pattern, deleted = deleted, "Pattern invalidation completed");

        // Publish invalidation event
        let event = InvalidationEvent {
            event_type: InvalidationType::Pattern,
            keys: vec![pattern],
            tags: Vec::new(),
            timestamp: chrono::Utc::now(),
            source_instance: self.config.instance_id.clone(),
        };

        self.publish_event(event).await?;

        Ok(())
    }

    /// Apply cascade invalidation rules
    fn apply_cascade_rules<'a>(
        &'a self,
        tag: &'a str,
        depth: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            if depth >= self.config.max_cascade_depth {
                warn!(tag = %tag, depth = depth, "Max cascade depth reached");
                return Ok(());
            }

            let matching_rules: Vec<_> = {
                let rules = self.cascade_rules.read();
                rules
                    .iter()
                    .filter(|rule| rule.source_tag == tag)
                    .cloned()
                    .collect()
            };

            for rule in matching_rules {
                debug!(
                    source = %rule.source_tag,
                    targets = ?rule.target_tags,
                    depth = depth,
                    "Applying cascade rule"
                );

                for target_tag in rule.target_tags {
                    let keys = self.tag_registry.get_keys_for_tag(&target_tag);

                    for key in keys {
                        if let Err(e) = self.cache.delete(&key).await {
                            error!(key = %key, error = %e, "Failed to cascade invalidate key");
                        }
                    }

                    // Recursively apply cascade rules
                    self.apply_cascade_rules(&target_tag, depth + 1).await?;
                }
            }

            Ok(())
        })
    }

    /// Publish invalidation event via Redis pub/sub
    async fn publish_event(&self, event: InvalidationEvent) -> Result<()> {
        let _json = serde_json::to_string(&event)
            .map_err(|e| DbError::Cache(format!("Serialization error: {}", e)))?;

        if let Err(e) = self.pubsub_tx.send(event) {
            error!(error = %e, "Failed to send invalidation event to channel");
        }

        debug!("Published invalidation event");

        Ok(())
    }

    /// Start pub/sub subscriber
    pub async fn start_subscriber(self: Arc<Self>, redis_url: String) -> Result<()> {
        let client = Client::open(redis_url.as_str())
            .map_err(|e| DbError::Connection(format!("Redis client error: {}", e)))?;

        let mut pubsub = client
            .get_async_pubsub()
            .await
            .map_err(|e| DbError::Connection(format!("Redis pubsub error: {}", e)))?;
        pubsub
            .subscribe(&self.config.pubsub_channel)
            .await
            .map_err(|e| DbError::Cache(format!("Subscribe error: {}", e)))?;

        info!(channel = %self.config.pubsub_channel, "Started invalidation subscriber");

        tokio::spawn(async move {
            loop {
                match pubsub.on_message().next().await {
                    Some(msg) => {
                        let payload: String = match msg.get_payload() {
                            Ok(p) => p,
                            Err(e) => {
                                error!(error = %e, "Failed to get message payload");
                                continue;
                            }
                        };

                        let event: InvalidationEvent = match serde_json::from_str(&payload) {
                            Ok(e) => e,
                            Err(e) => {
                                error!(error = %e, "Failed to deserialize event");
                                continue;
                            }
                        };

                        // Skip events from self
                        if event.source_instance == self.config.instance_id {
                            continue;
                        }

                        debug!(
                            event_type = ?event.event_type,
                            source = %event.source_instance,
                            "Received invalidation event"
                        );

                        // Process the event
                        match event.event_type {
                            InvalidationType::Keys => {
                                for key in &event.keys {
                                    if let Err(e) = self.cache.delete(key).await {
                                        error!(key = %key, error = %e, "Failed to invalidate key");
                                    }
                                }
                            }
                            InvalidationType::Tags => {
                                for tag in &event.tags {
                                    let keys = self.tag_registry.get_keys_for_tag(tag);
                                    for key in keys {
                                        if let Err(e) = self.cache.delete(&key).await {
                                            error!(key = %key, error = %e, "Failed to invalidate key");
                                        }
                                    }
                                }
                            }
                            InvalidationType::Pattern => {
                                for pattern in &event.keys {
                                    if let Err(e) = self.cache.delete_pattern(pattern).await {
                                        error!(pattern = %pattern, error = %e, "Failed to invalidate pattern");
                                    }
                                }
                            }
                            InvalidationType::Cascade => {
                                // Cascade handled by publisher
                            }
                        }
                    }
                    None => {
                        warn!("Pub/sub connection closed, reconnecting...");
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    }
                }
            }
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalidation_config_default() {
        let config = InvalidationConfig::default();
        assert_eq!(config.pubsub_channel, "cache:invalidation");
        assert!(config.enable_cascade);
        assert_eq!(config.max_cascade_depth, 5);
    }

    #[test]
    fn test_tag_registry_register() {
        let registry = TagRegistry::new();

        registry.register(
            "key1".to_string(),
            vec!["tag1".to_string(), "tag2".to_string()],
        );

        let keys = registry.get_keys_for_tag("tag1");
        assert_eq!(keys.len(), 1);
        assert!(keys.contains(&"key1".to_string()));
    }

    #[test]
    fn test_tag_registry_get_tags() {
        let registry = TagRegistry::new();

        registry.register(
            "key1".to_string(),
            vec!["tag1".to_string(), "tag2".to_string()],
        );

        let tags = registry.get_tags_for_key("key1");
        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&"tag1".to_string()));
        assert!(tags.contains(&"tag2".to_string()));
    }

    #[test]
    fn test_tag_registry_unregister() {
        let registry = TagRegistry::new();

        registry.register("key1".to_string(), vec!["tag1".to_string()]);
        registry.unregister("key1");

        let keys = registry.get_keys_for_tag("tag1");
        assert_eq!(keys.len(), 0);
    }

    #[test]
    fn test_cascade_rule_creation() {
        let rule = CascadeRule {
            source_tag: "user".to_string(),
            target_tags: vec!["user_profile".to_string(), "user_orders".to_string()],
        };

        assert_eq!(rule.source_tag, "user");
        assert_eq!(rule.target_tags.len(), 2);
    }

    #[test]
    fn test_invalidation_event_serialization() {
        let event = InvalidationEvent {
            event_type: InvalidationType::Keys,
            keys: vec!["key1".to_string()],
            tags: vec![],
            timestamp: chrono::Utc::now(),
            source_instance: "instance1".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: InvalidationEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.event_type, InvalidationType::Keys);
        assert_eq!(deserialized.keys.len(), 1);
    }
}
