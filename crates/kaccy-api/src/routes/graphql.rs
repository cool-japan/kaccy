//! GraphQL API routes

use async_graphql::{
    Context, EmptySubscription, ID, Object, Result as GraphQLResult, Schema, SimpleObject,
};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{Extension, response::IntoResponse};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::middleware::auth::Claims;

/// GraphQL User type
#[derive(SimpleObject, Clone)]
pub struct GqlUser {
    pub id: ID,
    pub username: String,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub reputation_score: i32,
    pub created_at: String,
}

/// GraphQL Token type
#[derive(SimpleObject, Clone)]
pub struct GqlToken {
    pub id: ID,
    pub symbol: String,
    pub name: String,
    pub description: Option<String>,
    pub total_supply: String,
    pub current_supply: String,
    pub issuer_id: ID,
    pub curve_type: String,
    pub initial_price: String,
    pub reserve_ratio: String,
    pub created_at: String,
}

/// GraphQL Order type
#[derive(SimpleObject, Clone)]
pub struct GqlOrder {
    pub id: ID,
    pub user_id: ID,
    pub token_id: ID,
    pub order_type: String,
    pub amount: String,
    pub price_btc: String,
    pub total_btc: String,
    pub status: String,
    pub created_at: String,
}

/// GraphQL Trade type
#[derive(SimpleObject, Clone)]
pub struct GqlTrade {
    pub id: ID,
    pub token_id: ID,
    pub buyer_id: ID,
    pub seller_id: ID,
    pub amount: String,
    pub price_btc: String,
    pub total_btc: String,
    pub created_at: String,
}

/// GraphQL Query root
pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Get current user
    async fn me(&self, ctx: &Context<'_>) -> GraphQLResult<Option<GqlUser>> {
        let claims = ctx.data_opt::<Claims>();
        if claims.is_none() {
            return Ok(None);
        }

        let claims = claims.unwrap();
        let pool = ctx.data::<PgPool>()?;
        let user_id = Uuid::parse_str(&claims.sub)?;

        let user = sqlx::query(
            "SELECT id, username, display_name, bio, avatar_url, reputation_score, created_at FROM users WHERE id = $1"
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        Ok(user.map(|row| GqlUser {
            id: ID(row.get::<Uuid, _>("id").to_string()),
            username: row.get("username"),
            display_name: row.get("display_name"),
            bio: row.get("bio"),
            avatar_url: row.get("avatar_url"),
            reputation_score: row.get("reputation_score"),
            created_at: row.get::<DateTime<Utc>, _>("created_at").to_rfc3339(),
        }))
    }

    /// Get user by ID
    async fn user(&self, ctx: &Context<'_>, id: ID) -> GraphQLResult<Option<GqlUser>> {
        let pool = ctx.data::<PgPool>()?;
        let user_id = Uuid::parse_str(id.as_ref())?;

        let user = sqlx::query(
            "SELECT id, username, display_name, bio, avatar_url, reputation_score, created_at FROM users WHERE id = $1"
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        Ok(user.map(|row| GqlUser {
            id: ID(row.get::<Uuid, _>("id").to_string()),
            username: row.get("username"),
            display_name: row.get("display_name"),
            bio: row.get("bio"),
            avatar_url: row.get("avatar_url"),
            reputation_score: row.get("reputation_score"),
            created_at: row.get::<DateTime<Utc>, _>("created_at").to_rfc3339(),
        }))
    }

    /// Get token by ID
    async fn token(&self, ctx: &Context<'_>, id: ID) -> GraphQLResult<Option<GqlToken>> {
        let pool = ctx.data::<PgPool>()?;
        let token_id = Uuid::parse_str(id.as_ref())?;

        let token = sqlx::query(
            "SELECT id, symbol, name, description, total_supply, current_supply, issuer_id, curve_type, initial_price, reserve_ratio, created_at FROM tokens WHERE id = $1"
        )
        .bind(token_id)
        .fetch_optional(pool)
        .await?;

        Ok(token.map(|row| GqlToken {
            id: ID(row.get::<Uuid, _>("id").to_string()),
            symbol: row.get("symbol"),
            name: row.get("name"),
            description: row.get("description"),
            total_supply: row
                .get::<rust_decimal::Decimal, _>("total_supply")
                .to_string(),
            current_supply: row
                .get::<rust_decimal::Decimal, _>("current_supply")
                .to_string(),
            issuer_id: ID(row.get::<Uuid, _>("issuer_id").to_string()),
            curve_type: row.get("curve_type"),
            initial_price: row
                .get::<rust_decimal::Decimal, _>("initial_price")
                .to_string(),
            reserve_ratio: row
                .get::<rust_decimal::Decimal, _>("reserve_ratio")
                .to_string(),
            created_at: row.get::<DateTime<Utc>, _>("created_at").to_rfc3339(),
        }))
    }

    /// List all tokens
    async fn tokens(
        &self,
        ctx: &Context<'_>,
        #[graphql(default = 20)] limit: i32,
        #[graphql(default = 0)] offset: i32,
    ) -> GraphQLResult<Vec<GqlToken>> {
        let pool = ctx.data::<PgPool>()?;

        let tokens = sqlx::query(
            "SELECT id, symbol, name, description, total_supply, current_supply, issuer_id, curve_type, initial_price, reserve_ratio, created_at FROM tokens ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(pool)
        .await?;

        Ok(tokens
            .into_iter()
            .map(|row| GqlToken {
                id: ID(row.get::<Uuid, _>("id").to_string()),
                symbol: row.get("symbol"),
                name: row.get("name"),
                description: row.get("description"),
                total_supply: row.get::<Decimal, _>("total_supply").to_string(),
                current_supply: row.get::<Decimal, _>("current_supply").to_string(),
                issuer_id: ID(row.get::<Uuid, _>("issuer_id").to_string()),
                curve_type: row.get("curve_type"),
                initial_price: row.get::<Decimal, _>("initial_price").to_string(),
                reserve_ratio: row.get::<Decimal, _>("reserve_ratio").to_string(),
                created_at: row.get::<DateTime<Utc>, _>("created_at").to_rfc3339(),
            })
            .collect())
    }

    /// Get user's orders
    async fn my_orders(
        &self,
        ctx: &Context<'_>,
        #[graphql(default = 20)] limit: i32,
    ) -> GraphQLResult<Vec<GqlOrder>> {
        let claims = ctx.data_opt::<Claims>();
        if claims.is_none() {
            return Err("Unauthorized".into());
        }

        let claims = claims.unwrap();
        let pool = ctx.data::<PgPool>()?;
        let user_id = Uuid::parse_str(&claims.sub)?;

        let orders = sqlx::query(
            "SELECT id, user_id, token_id, order_type, amount, price_btc, total_btc, status, created_at FROM orders WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2"
        )
        .bind(user_id)
        .bind(limit as i64)
        .fetch_all(pool)
        .await?;

        Ok(orders
            .into_iter()
            .map(|row| GqlOrder {
                id: ID(row.get::<Uuid, _>("id").to_string()),
                user_id: ID(row.get::<Uuid, _>("user_id").to_string()),
                token_id: ID(row.get::<Uuid, _>("token_id").to_string()),
                order_type: row.get("order_type"),
                amount: row.get::<Decimal, _>("amount").to_string(),
                price_btc: row.get::<Decimal, _>("price_btc").to_string(),
                total_btc: row.get::<Decimal, _>("total_btc").to_string(),
                status: row.get("status"),
                created_at: row.get::<DateTime<Utc>, _>("created_at").to_rfc3339(),
            })
            .collect())
    }

    /// Get trades for a token
    async fn token_trades(
        &self,
        ctx: &Context<'_>,
        token_id: ID,
        #[graphql(default = 20)] limit: i32,
    ) -> GraphQLResult<Vec<GqlTrade>> {
        let pool = ctx.data::<PgPool>()?;
        let token_uuid = Uuid::parse_str(token_id.as_ref())?;

        let trades = sqlx::query(
            "SELECT id, token_id, buyer_id, seller_id, amount, price_btc, total_btc, created_at FROM trades WHERE token_id = $1 ORDER BY created_at DESC LIMIT $2"
        )
        .bind(token_uuid)
        .bind(limit as i64)
        .fetch_all(pool)
        .await?;

        Ok(trades
            .into_iter()
            .map(|row| GqlTrade {
                id: ID(row.get::<Uuid, _>("id").to_string()),
                token_id: ID(row.get::<Uuid, _>("token_id").to_string()),
                buyer_id: ID(row.get::<Uuid, _>("buyer_id").to_string()),
                seller_id: ID(row.get::<Uuid, _>("seller_id").to_string()),
                amount: row.get::<Decimal, _>("amount").to_string(),
                price_btc: row.get::<Decimal, _>("price_btc").to_string(),
                total_btc: row.get::<Decimal, _>("total_btc").to_string(),
                created_at: row.get::<DateTime<Utc>, _>("created_at").to_rfc3339(),
            })
            .collect())
    }
}

/// GraphQL Mutation root
pub struct MutationRoot;

#[Object]
impl MutationRoot {
    /// Update user profile
    async fn update_profile(
        &self,
        ctx: &Context<'_>,
        display_name: Option<String>,
        bio: Option<String>,
        avatar_url: Option<String>,
    ) -> GraphQLResult<GqlUser> {
        let claims = ctx.data_opt::<Claims>();
        if claims.is_none() {
            return Err("Unauthorized".into());
        }

        let claims = claims.unwrap();
        let pool = ctx.data::<PgPool>()?;
        let user_id = Uuid::parse_str(&claims.sub)?;

        let user = sqlx::query(
            "UPDATE users SET display_name = COALESCE($1, display_name), bio = COALESCE($2, bio), avatar_url = COALESCE($3, avatar_url), updated_at = NOW() WHERE id = $4 RETURNING id, username, display_name, bio, avatar_url, reputation_score, created_at"
        )
        .bind(display_name)
        .bind(bio)
        .bind(avatar_url)
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        Ok(GqlUser {
            id: ID(user.get::<Uuid, _>("id").to_string()),
            username: user.get("username"),
            display_name: user.get("display_name"),
            bio: user.get("bio"),
            avatar_url: user.get("avatar_url"),
            reputation_score: user.get("reputation_score"),
            created_at: user.get::<DateTime<Utc>, _>("created_at").to_rfc3339(),
        })
    }
}

/// GraphQL Schema type
pub type KaccySchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

/// Create GraphQL schema
pub fn create_schema(pool: PgPool) -> KaccySchema {
    Schema::build(QueryRoot, MutationRoot, EmptySubscription)
        .data(pool)
        .finish()
}

/// GraphQL query handler
pub async fn graphql_handler(
    schema: Extension<KaccySchema>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

/// GraphQL playground handler
pub async fn graphql_playground() -> impl IntoResponse {
    async_graphql::http::GraphiQLSource::build()
        .endpoint("/api/graphql")
        .finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gql_types() {
        let user = GqlUser {
            id: ID("test-id".to_string()),
            username: "testuser".to_string(),
            display_name: Some("Test User".to_string()),
            bio: None,
            avatar_url: None,
            reputation_score: 500,
            created_at: Utc::now().to_rfc3339(),
        };

        assert_eq!(user.username, "testuser");
        assert_eq!(user.reputation_score, 500);
    }
}
