//! Streaming responses for large datasets

use axum::{
    extract::{Query, State},
    response::Response,
};
use futures::{
    StreamExt,
    stream::{self},
};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use uuid::Uuid;

use crate::{
    error::{ApiError, Result},
    middleware::Claims,
    state::AppState,
};

/// Query parameters for streaming
#[derive(Debug, Deserialize)]
pub struct StreamQuery {
    /// Page size for chunked streaming
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,
    /// Optional cursor for pagination
    pub cursor: Option<String>,
}

fn default_chunk_size() -> usize {
    100
}

// Note: SSE functions are omitted as they require the `sse` feature in axum
// which may not be available. Standard HTTP streaming is provided below.

/// Newline-delimited JSON (NDJSON) streaming
/// Each line is a separate JSON object
#[derive(Debug, Serialize)]
pub struct TokenData {
    pub id: Uuid,
    pub symbol: String,
    pub name: String,
    pub price_btc: f64,
}

/// Stream tokens as NDJSON
pub async fn stream_tokens_ndjson(
    State(_state): State<AppState>,
    Query(query): Query<StreamQuery>,
) -> Response {
    let chunk_size = query.chunk_size.min(1000);
    let total_items = 500;

    // Create a stream of tokens
    let stream = stream::iter(0..total_items)
        .map(move |i| {
            let token = TokenData {
                id: Uuid::new_v4(),
                symbol: format!("TOKEN{}", i),
                name: format!("Token {}", i),
                price_btc: 0.001 * (i as f64),
            };

            // Serialize to JSON and add newline
            let json = serde_json::to_string(&token).unwrap();
            format!("{}\n", json)
        })
        .chunks(chunk_size)
        .map(|chunk| chunk.join(""))
        .map(Ok::<_, Infallible>);

    let body = axum::body::Body::from_stream(stream);

    Response::builder()
        .header("Content-Type", "application/x-ndjson")
        .header("Transfer-Encoding", "chunked")
        .body(body)
        .unwrap()
}

/// JSON Lines streaming (similar to NDJSON)
pub async fn stream_orders_jsonlines(
    State(_state): State<AppState>,
    claims: Claims,
    Query(query): Query<StreamQuery>,
) -> Result<Response> {
    let _user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| ApiError::Auth("Invalid user ID in token".to_string()))?;

    let chunk_size = query.chunk_size.min(1000);
    let total_items = 300;

    let stream = stream::iter(0..total_items)
        .map(move |i| {
            let order = serde_json::json!({
                "id": Uuid::new_v4(),
                "type": if i % 2 == 0 { "buy" } else { "sell" },
                "amount": 100.0 + (i as f64),
                "status": "pending",
            });

            let json = serde_json::to_string(&order).unwrap();
            format!("{}\n", json)
        })
        .chunks(chunk_size)
        .map(|chunk| chunk.join(""))
        .map(Ok::<_, Infallible>);

    let body = axum::body::Body::from_stream(stream);

    Ok(Response::builder()
        .header("Content-Type", "application/jsonlines")
        .header("Transfer-Encoding", "chunked")
        .body(body)
        .unwrap())
}

/// Chunked JSON array streaming
pub async fn stream_tokens_chunked_json(
    State(_state): State<AppState>,
    Query(query): Query<StreamQuery>,
) -> Response {
    let chunk_size = query.chunk_size.min(1000);
    let total_items = 500;

    // Stream opening bracket
    let header = stream::once(async { Ok::<_, Infallible>("[".to_string()) });

    // Stream items
    let items = stream::iter(0..total_items)
        .enumerate()
        .map(move |(idx, i)| {
            let token = TokenData {
                id: Uuid::new_v4(),
                symbol: format!("TOKEN{}", i),
                name: format!("Token {}", i),
                price_btc: 0.001 * (i as f64),
            };

            let json = serde_json::to_string(&token).unwrap();
            if idx == 0 { json } else { format!(",{}", json) }
        })
        .chunks(chunk_size)
        .map(|chunk| Ok::<_, Infallible>(chunk.join("")));

    // Stream closing bracket
    let footer = stream::once(async { Ok::<_, Infallible>("]".to_string()) });

    // Combine all parts
    let stream = header.chain(items).chain(footer);

    let body = axum::body::Body::from_stream(stream);

    Response::builder()
        .header("Content-Type", "application/json")
        .header("Transfer-Encoding", "chunked")
        .body(body)
        .unwrap()
}

/// Export large dataset as CSV stream
pub async fn export_tokens_csv(
    State(_state): State<AppState>,
    Query(query): Query<StreamQuery>,
) -> Response {
    let chunk_size = query.chunk_size.min(1000);
    let total_items = 1000;

    // CSV header
    let header =
        stream::once(async { Ok::<_, Infallible>("id,symbol,name,price_btc\n".to_string()) });

    // CSV rows
    let rows = stream::iter(0..total_items)
        .map(move |i| {
            let symbol = format!("TOKEN{}", i);
            let name = format!("Token {}", i);
            format!(
                "{},{},{},{}\n",
                Uuid::new_v4(),
                symbol,
                name,
                0.001 * (i as f64)
            )
        })
        .chunks(chunk_size)
        .map(|chunk| Ok::<_, Infallible>(chunk.join("")));

    let stream = header.chain(rows);
    let body = axum::body::Body::from_stream(stream);

    Response::builder()
        .header("Content-Type", "text/csv")
        .header("Content-Disposition", "attachment; filename=\"tokens.csv\"")
        .header("Transfer-Encoding", "chunked")
        .body(body)
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_chunk_size() {
        assert_eq!(default_chunk_size(), 100);
    }

    #[test]
    fn test_stream_query_defaults() {
        let query = StreamQuery {
            chunk_size: default_chunk_size(),
            cursor: None,
        };

        assert_eq!(query.chunk_size, 100);
        assert!(query.cursor.is_none());
    }

    #[test]
    fn test_token_data_serialization() {
        let token = TokenData {
            id: Uuid::new_v4(),
            symbol: "BTC".to_string(),
            name: "Bitcoin".to_string(),
            price_btc: 1.0,
        };

        let json = serde_json::to_string(&token).unwrap();
        assert!(json.contains("BTC"));
        assert!(json.contains("Bitcoin"));
    }
}
