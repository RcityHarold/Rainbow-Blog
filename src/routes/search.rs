use crate::{error::Result, state::AppState};
use axum::{response::Json, routing::get, Router};
use serde_json::{json, Value};
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(placeholder_handler))
}

async fn placeholder_handler() -> Result<Json<Value>> {
    Ok(Json(json!({
        "success": true,
        "message": "Search API - Coming soon",
        "endpoints": [
            "GET /api/search?q=query - Global search",
            "GET /api/search/articles?q=query - Search articles",
            "GET /api/search/users?q=query - Search users",
            "GET /api/search/tags?q=query - Search tags"
        ]
    })))
}