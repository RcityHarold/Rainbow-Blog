use crate::{
    error::{Result, AppError},
    models::tag::*,
    services::auth::User,
    state::AppState,
    require_permission,
    utils::middleware::OptionalAuth,
};
use axum::{
    extract::{Path, Query, State},
    response::Json,
    routing::{delete, get, post, put},
    Extension, Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::debug;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_tags).post(create_tag))
        .route("/:id", put(update_tag).delete(delete_tag))
        .route("/slug/:slug", get(get_tag_by_slug))
        .route("/article/:article_id", get(get_article_tags))
        .route("/article/:article_id/tags", post(add_article_tags).delete(remove_article_tags))
        .route("/:id/follow", post(follow_tag).delete(unfollow_tag))
        .route("/followed", get(get_user_followed_tags))
}

/// Get all tags
/// GET /api/tags
async fn get_tags(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TagQuery>,
    OptionalAuth(user): OptionalAuth,
) -> Result<Json<Value>> {
    debug!("Getting tags with query: {:?}", query);

    let tags = state.tag_service.get_tags(query).await?;

    // If user is authenticated, get follow status
    let tags_response = if let Some(user) = user {
        let tag_ids: Vec<String> = tags.iter().map(|t| t.id.clone()).collect();
        state
            .tag_service
            .get_tags_with_follow_status(Some(&user.id), tag_ids)
            .await?
    } else {
        tags.into_iter()
            .map(|tag| TagWithFollowStatus {
                tag,
                is_following: false,
            })
            .collect()
    };

    Ok(Json(json!({
        "success": true,
        "data": tags_response
    })))
}

/// Create a new tag (admin only)
/// POST /api/tags
async fn create_tag(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Json(request): Json<CreateTagRequest>,
) -> Result<Json<Value>> {
    debug!("Creating tag: {}", request.name);

    // Check permissions
    require_permission!(state.auth_service, user, "tag.create");

    let tag = state.tag_service.create_tag(request).await?;

    Ok(Json(json!({
        "success": true,
        "data": tag,
        "message": "Tag created successfully"
    })))
}

/// Update a tag (admin only)
/// PUT /api/tags/:id
async fn update_tag(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(tag_id): Path<String>,
    Json(request): Json<UpdateTagRequest>,
) -> Result<Json<Value>> {
    debug!("Updating tag: {}", tag_id);

    // Check permissions
    require_permission!(state.auth_service, user, "tag.update");

    let tag = state.tag_service.update_tag(&tag_id, request).await?;

    Ok(Json(json!({
        "success": true,
        "data": tag,
        "message": "Tag updated successfully"
    })))
}

/// Delete a tag (admin only)
/// DELETE /api/tags/:id
async fn delete_tag(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(tag_id): Path<String>,
) -> Result<Json<Value>> {
    debug!("Deleting tag: {}", tag_id);

    // Check permissions
    require_permission!(state.auth_service, user, "tag.delete");

    state.tag_service.delete_tag(&tag_id).await?;

    Ok(Json(json!({
        "success": true,
        "message": "Tag deleted successfully"
    })))
}

/// Get tag by slug
/// GET /api/tags/slug/:slug
async fn get_tag_by_slug(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    OptionalAuth(user): OptionalAuth,
) -> Result<Json<Value>> {
    debug!("Getting tag by slug: {}", slug);

    let tag = state
        .tag_service
        .get_tag_by_slug(&slug)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound("Tag not found".to_string()))?;

    let tag_response = if let Some(user) = user {
        let tags_with_status = state
            .tag_service
            .get_tags_with_follow_status(Some(&user.id), vec![tag.id.clone()])
            .await?;
        
        tags_with_status.into_iter().next().unwrap()
    } else {
        TagWithFollowStatus {
            tag,
            is_following: false,
        }
    };

    Ok(Json(json!({
        "success": true,
        "data": tag_response
    })))
}

/// Get tags for an article
/// GET /api/tags/article/:article_id
async fn get_article_tags(
    State(state): State<Arc<AppState>>,
    Path(article_id): Path<String>,
) -> Result<Json<Value>> {
    debug!("Getting tags for article: {}", article_id);

    let tags = state.tag_service.get_article_tags(&article_id).await?;

    Ok(Json(json!({
        "success": true,
        "data": tags
    })))
}

/// Add tags to an article
/// POST /api/tags/article/:article_id/tags
async fn add_article_tags(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(article_id): Path<String>,
    Json(tag_ids): Json<Vec<String>>,
) -> Result<Json<Value>> {
    debug!("Adding {} tags to article: {}", tag_ids.len(), article_id);

    // Verify user owns the article
    let article = state.article_service.get_article_by_id(&article_id).await?
        .ok_or_else(|| AppError::NotFound("Article not found".to_string()))?;
    
    if article.author_id != user.id {
        return Err(AppError::forbidden("You can only modify tags on your own articles"));
    }
    
    state
        .tag_service
        .add_tags_to_article(&article_id, tag_ids)
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "Tags added successfully"
    })))
}

/// Remove tags from an article
/// DELETE /api/tags/article/:article_id/tags
async fn remove_article_tags(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(article_id): Path<String>,
    Json(tag_ids): Json<Vec<String>>,
) -> Result<Json<Value>> {
    debug!("Removing {} tags from article: {}", tag_ids.len(), article_id);

    // Verify user owns the article
    let article = state.article_service.get_article_by_id(&article_id).await?
        .ok_or_else(|| AppError::NotFound("Article not found".to_string()))?;
    
    if article.author_id != user.id {
        return Err(AppError::forbidden("You can only modify tags on your own articles"));
    }
    
    state
        .tag_service
        .remove_tags_from_article(&article_id, tag_ids)
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "Tags removed successfully"
    })))
}

/// Follow a tag
/// POST /api/tags/:id/follow
async fn follow_tag(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(tag_id): Path<String>,
) -> Result<Json<Value>> {
    debug!("User {} following tag: {}", user.id, tag_id);

    state.tag_service.follow_tag(&tag_id, &user.id).await?;

    Ok(Json(json!({
        "success": true,
        "message": "Tag followed successfully"
    })))
}

/// Unfollow a tag
/// DELETE /api/tags/:id/follow
async fn unfollow_tag(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(tag_id): Path<String>,
) -> Result<Json<Value>> {
    debug!("User {} unfollowing tag: {}", user.id, tag_id);

    state.tag_service.unfollow_tag(&tag_id, &user.id).await?;

    Ok(Json(json!({
        "success": true,
        "message": "Tag unfollowed successfully"
    })))
}

/// Get user's followed tags
/// GET /api/tags/followed
async fn get_user_followed_tags(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
) -> Result<Json<Value>> {
    debug!("Getting followed tags for user: {}", user.id);

    let tags = state
        .tag_service
        .get_user_followed_tags(&user.id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": tags
    })))
}