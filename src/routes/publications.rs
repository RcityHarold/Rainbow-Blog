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
        "message": "Publications API - Coming soon",
        "endpoints": [
            "GET /api/publications - Get all publications",
            "POST /api/publications - Create publication",
            "GET /api/publications/:slug - Get publication details",
            "GET /api/publications/:slug/articles - Get publication articles"
        ]
    })))
}