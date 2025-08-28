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
        "message": "Statistics API - Coming soon",
        "endpoints": [
            "GET /api/stats/dashboard - Dashboard statistics",
            "GET /api/stats/articles - Article statistics",
            "GET /api/stats/users - User statistics"
        ]
    })))
}