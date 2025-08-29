use crate::{
    error::Result,
    models::comment::*,
    services::AuthService,
    state::AppState,
    utils::middleware::OptionalAuth,
};
use axum::{
    extract::{Path, State},
    response::Json,
    routing::{delete, get, post, put},
    Extension, Router,
};
use serde_json::{json, Value};
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/article/:article_id", get(get_article_comments))
        .route("/", post(create_comment))
        .route("/:id", put(update_comment))
        .route("/:id", delete(delete_comment))
        .route("/:id/clap", post(clap_comment))
        .route("/:id/clap", delete(remove_clap))
}

async fn get_article_comments(
    State(state): State<Arc<AppState>>,
    Path(article_id): Path<String>,
    OptionalAuth(user): OptionalAuth,
) -> Result<Json<Value>> {
    let user_id = user.as_ref().map(|u| u.id.as_str());
    let comments = state
        .comment_service
        .get_article_comments(&article_id, user_id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": comments
    })))
}

async fn create_comment(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::services::auth::User>,
    Json(request): Json<CreateCommentRequest>,
) -> Result<Json<Value>> {
    let comment = state
        .comment_service
        .create_comment(&user.id, request)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": comment
    })))
}

async fn update_comment(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::services::auth::User>,
    Path(comment_id): Path<String>,
    Json(request): Json<UpdateCommentRequest>,
) -> Result<Json<Value>> {
    let comment = state
        .comment_service
        .update_comment(&comment_id, &user.id, request)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": comment
    })))
}

async fn delete_comment(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::services::auth::User>,
    Path(comment_id): Path<String>,
) -> Result<Json<Value>> {
    state
        .comment_service
        .delete_comment(&comment_id, &user.id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "Comment deleted successfully"
    })))
}

async fn clap_comment(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::services::auth::User>,
    Path(comment_id): Path<String>,
) -> Result<Json<Value>> {
    state
        .comment_service
        .clap_comment(&comment_id, &user.id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "Comment clapped successfully"
    })))
}

async fn remove_clap(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::services::auth::User>,
    Path(comment_id): Path<String>,
) -> Result<Json<Value>> {
    state
        .comment_service
        .remove_clap(&comment_id, &user.id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "Clap removed successfully"
    })))
}