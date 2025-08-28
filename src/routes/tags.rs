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
        "message": "Tags API - Coming soon",
        "endpoints": [
            "GET /api/tags - Get all tags",
            "GET /api/tags/:slug - Get tag details",
            "GET /api/tags/:slug/articles - Get articles by tag"
        ]
    })))
}