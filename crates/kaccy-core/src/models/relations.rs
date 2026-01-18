//! Model relationship helpers for eager loading and relation queries

use sqlx::PgPool;
use uuid::Uuid;

use crate::error::Result;

/// Trait for models that support eager loading of relations
pub trait EagerLoad: Sized {
    /// The type of the relation to load
    type Relation;

    /// Load a specific relation
    fn load_relation(
        &mut self,
        relation_name: &str,
        pool: &PgPool,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
}

/// Trait for models that have relations
pub trait HasRelations {
    /// Get the list of available relation names
    fn relation_names() -> Vec<&'static str>;
}

/// Relation query builder
#[derive(Debug, Clone)]
pub struct RelationQuery {
    /// Relations to eagerly load
    pub relations: Vec<String>,
}

impl RelationQuery {
    /// Create a new relation query
    pub fn new() -> Self {
        Self {
            relations: Vec::new(),
        }
    }

    /// Add a relation to load
    pub fn with(mut self, relation: impl Into<String>) -> Self {
        self.relations.push(relation.into());
        self
    }

    /// Add multiple relations to load
    pub fn with_many<I, S>(mut self, relations: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.relations.extend(relations.into_iter().map(Into::into));
        self
    }

    /// Check if a relation should be loaded
    pub fn should_load(&self, relation: &str) -> bool {
        self.relations.iter().any(|r| r == relation)
    }
}

impl Default for RelationQuery {
    fn default() -> Self {
        Self::new()
    }
}

/// Cascade delete configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CascadeAction {
    /// Delete related records
    Delete,
    /// Set foreign key to NULL
    SetNull,
    /// Prevent deletion if related records exist
    Restrict,
    /// No action (database will handle it)
    NoAction,
}

/// Trait for models that support cascade operations
#[async_trait::async_trait]
pub trait CascadeDelete {
    /// Delete the model and handle related records according to cascade rules
    async fn cascade_delete(&self, pool: &PgPool) -> Result<u64>;

    /// Get cascade configuration for each relation
    fn cascade_config() -> Vec<(&'static str, CascadeAction)> {
        Vec::new()
    }
}

/// Helper for loading user relations
pub struct UserRelations;

impl UserRelations {
    /// Load user's tokens
    pub async fn load_tokens(user_id: Uuid, pool: &PgPool) -> Result<Vec<crate::models::Token>> {
        let tokens = sqlx::query_as::<_, crate::models::Token>(
            "SELECT * FROM tokens WHERE issuer_user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(tokens)
    }

    /// Load user's orders
    pub async fn load_orders(user_id: Uuid, pool: &PgPool) -> Result<Vec<crate::models::Order>> {
        let orders = sqlx::query_as::<_, crate::models::Order>(
            "SELECT * FROM orders WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(orders)
    }

    /// Load user's balances
    pub async fn load_balances(
        user_id: Uuid,
        pool: &PgPool,
    ) -> Result<Vec<crate::models::Balance>> {
        let balances = sqlx::query_as::<_, crate::models::Balance>(
            "SELECT * FROM balances WHERE user_id = $1 AND amount > 0 ORDER BY updated_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(balances)
    }
}

/// Helper for loading token relations
pub struct TokenRelations;

impl TokenRelations {
    /// Load token's issuer
    pub async fn load_issuer(token_id: Uuid, pool: &PgPool) -> Result<Option<crate::models::User>> {
        let issuer = sqlx::query_as::<_, crate::models::User>(
            "SELECT u.* FROM users u
             JOIN tokens t ON u.user_id = t.issuer_user_id
             WHERE t.token_id = $1",
        )
        .bind(token_id)
        .fetch_optional(pool)
        .await?;

        Ok(issuer)
    }

    /// Load token's orders
    pub async fn load_orders(token_id: Uuid, pool: &PgPool) -> Result<Vec<crate::models::Order>> {
        let orders = sqlx::query_as::<_, crate::models::Order>(
            "SELECT * FROM orders WHERE token_id = $1 ORDER BY created_at DESC",
        )
        .bind(token_id)
        .fetch_all(pool)
        .await?;

        Ok(orders)
    }

    /// Load token's trades
    pub async fn load_trades(token_id: Uuid, pool: &PgPool) -> Result<Vec<crate::models::Trade>> {
        let trades = sqlx::query_as::<_, crate::models::Trade>(
            "SELECT * FROM trades WHERE token_id = $1 ORDER BY executed_at DESC",
        )
        .bind(token_id)
        .fetch_all(pool)
        .await?;

        Ok(trades)
    }
}

/// Helper for loading order relations
pub struct OrderRelations;

impl OrderRelations {
    /// Load order's user
    pub async fn load_user(order_id: Uuid, pool: &PgPool) -> Result<Option<crate::models::User>> {
        let user = sqlx::query_as::<_, crate::models::User>(
            "SELECT u.* FROM users u
             JOIN orders o ON u.user_id = o.user_id
             WHERE o.order_id = $1",
        )
        .bind(order_id)
        .fetch_optional(pool)
        .await?;

        Ok(user)
    }

    /// Load order's token
    pub async fn load_token(order_id: Uuid, pool: &PgPool) -> Result<Option<crate::models::Token>> {
        let token = sqlx::query_as::<_, crate::models::Token>(
            "SELECT t.* FROM tokens t
             JOIN orders o ON t.token_id = o.token_id
             WHERE o.order_id = $1",
        )
        .bind(order_id)
        .fetch_optional(pool)
        .await?;

        Ok(token)
    }
}

/// Helper for loading trade relations
pub struct TradeRelations;

impl TradeRelations {
    /// Load trade's buyer
    pub async fn load_buyer(trade_id: Uuid, pool: &PgPool) -> Result<Option<crate::models::User>> {
        let buyer = sqlx::query_as::<_, crate::models::User>(
            "SELECT u.* FROM users u
             JOIN trades t ON u.user_id = t.buyer_user_id
             WHERE t.trade_id = $1",
        )
        .bind(trade_id)
        .fetch_optional(pool)
        .await?;

        Ok(buyer)
    }

    /// Load trade's seller
    pub async fn load_seller(trade_id: Uuid, pool: &PgPool) -> Result<Option<crate::models::User>> {
        let seller = sqlx::query_as::<_, crate::models::User>(
            "SELECT u.* FROM users u
             JOIN trades t ON u.user_id = t.seller_user_id
             WHERE t.trade_id = $1 AND t.seller_user_id IS NOT NULL",
        )
        .bind(trade_id)
        .fetch_optional(pool)
        .await?;

        Ok(seller)
    }

    /// Load trade's token
    pub async fn load_token(trade_id: Uuid, pool: &PgPool) -> Result<Option<crate::models::Token>> {
        let token = sqlx::query_as::<_, crate::models::Token>(
            "SELECT t.* FROM tokens t
             JOIN trades tr ON t.token_id = tr.token_id
             WHERE tr.trade_id = $1",
        )
        .bind(trade_id)
        .fetch_optional(pool)
        .await?;

        Ok(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relation_query() {
        let query = RelationQuery::new().with("tokens").with("orders");

        assert!(query.should_load("tokens"));
        assert!(query.should_load("orders"));
        assert!(!query.should_load("balances"));
    }

    #[test]
    fn test_relation_query_with_many() {
        let relations = vec!["tokens", "orders", "balances"];
        let query = RelationQuery::new().with_many(relations);

        assert_eq!(query.relations.len(), 3);
        assert!(query.should_load("tokens"));
        assert!(query.should_load("orders"));
        assert!(query.should_load("balances"));
    }

    #[test]
    fn test_cascade_action() {
        assert_eq!(CascadeAction::Delete, CascadeAction::Delete);
        assert_ne!(CascadeAction::Delete, CascadeAction::SetNull);
    }
}
