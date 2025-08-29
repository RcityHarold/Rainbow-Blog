use crate::{
    error::Result,
    models::bookmark::*,
    services::auth::User,
    state::AppState,
};
use axum::{
    extract::{Path, Query, State},
    response::Json,
    routing::{delete, get, post, put},
    Extension, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::debug;

#[derive(Debug, Deserialize)]
pub struct BookmarkQuery {
    pub page: Option<i32>,
    pub limit: Option<i32>,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_user_bookmarks).post(create_bookmark))
        .route("/:id", put(update_bookmark).delete(delete_bookmark))
        .route("/article/:article_id", delete(delete_by_article))
        .route("/check/:article_id", get(check_bookmark))
}

/// Get user's bookmarks
/// GET /api/bookmarks
async fn get_user_bookmarks(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Query(query): Query<BookmarkQuery>,
) -> Result<Json<Value>> {
    debug!("Getting bookmarks for user: {}", user.id);

    let bookmarks = state
        .bookmark_service
        .get_user_bookmarks(&user.id, query.page, query.limit)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": bookmarks
    })))
}

/// Create a bookmark
/// POST /api/bookmarks
async fn create_bookmark(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Json(request): Json<CreateBookmarkRequest>,
) -> Result<Json<Value>> {
    debug!("Creating bookmark for article: {} by user: {}", request.article_id, user.id);

    let bookmark = state
        .bookmark_service
        .create_bookmark(&user.id, request)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": bookmark,
        "message": "Bookmark created successfully"
    })))
}

/// Update a bookmark's note
/// PUT /api/bookmarks/:id
async fn update_bookmark(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(bookmark_id): Path<String>,
    Json(request): Json<UpdateBookmarkRequest>,
) -> Result<Json<Value>> {
    debug!("Updating bookmark: {} by user: {}", bookmark_id, user.id);

    let bookmark = state
        .bookmark_service
        .update_bookmark(&bookmark_id, &user.id, request)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": bookmark,
        "message": "Bookmark updated successfully"
    })))
}

/// Delete a bookmark
/// DELETE /api/bookmarks/:id
async fn delete_bookmark(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(bookmark_id): Path<String>,
) -> Result<Json<Value>> {
    debug!("Deleting bookmark: {} by user: {}", bookmark_id, user.id);

    state
        .bookmark_service
        .delete_bookmark(&bookmark_id, &user.id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "Bookmark deleted successfully"
    })))
}

/// Delete a bookmark by article ID
/// DELETE /api/bookmarks/article/:article_id
async fn delete_by_article(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(article_id): Path<String>,
) -> Result<Json<Value>> {
    debug!("Deleting bookmark for article: {} by user: {}", article_id, user.id);

    state
        .bookmark_service
        .delete_bookmark_by_article(&article_id, &user.id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "Bookmark removed successfully"
    })))
}

/// Check if an article is bookmarked
/// GET /api/bookmarks/check/:article_id
async fn check_bookmark(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(article_id): Path<String>,
) -> Result<Json<Value>> {
    debug!("Checking bookmark status for article: {} by user: {}", article_id, user.id);

    let is_bookmarked = state
        .bookmark_service
        .is_bookmarked(&article_id, &user.id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": {
            "is_bookmarked": is_bookmarked
        }
    })))
}