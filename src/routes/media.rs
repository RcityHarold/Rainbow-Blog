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
        "message": "Media API - Coming soon",
        "endpoints": [
            "POST /api/media/upload - Upload image",
            "GET /api/media/:id - Get media file",
            "DELETE /api/media/:id - Delete media file"
        ]
    })))
}