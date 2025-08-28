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
        "message": "Comments API - Coming soon",
        "endpoints": [
            "GET /api/comments/:article_id - Get article comments",
            "POST /api/comments - Create comment",
            "PUT /api/comments/:id - Update comment",
            "DELETE /api/comments/:id - Delete comment"
        ]
    })))
}