use crate::{
    error::Result,
    models::follow::*,
    services::auth::User,
    state::AppState,
    utils::middleware::OptionalAuth,
};
use axum::{
    extract::{Path, Query, State},
    response::Json,
    routing::{delete, get, post},
    Extension, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::debug;

#[derive(Debug, Deserialize)]
pub struct FollowQuery {
    pub page: Option<i32>,
    pub limit: Option<i32>,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/user/:user_id/follow", post(follow_user).delete(unfollow_user))
        .route("/user/:user_id/followers", get(get_followers))
        .route("/user/:user_id/following", get(get_following))
        .route("/user/:user_id/stats", get(get_follow_stats))
        .route("/user/:user_id/is-following", get(check_following))
        .route("/mutual/:target_user_id", get(get_mutual_followers))
}

/// 关注用户
/// POST /api/follows/user/:user_id/follow
async fn follow_user(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(user_id): Path<String>,
) -> Result<Json<Value>> {
    debug!("User {} following user {}", user.id, user_id);

    state
        .follow_service
        .follow_user(&user.id, &user_id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "User followed successfully"
    })))
}

/// 取消关注用户
/// DELETE /api/follows/user/:user_id/follow
async fn unfollow_user(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(user_id): Path<String>,
) -> Result<Json<Value>> {
    debug!("User {} unfollowing user {}", user.id, user_id);

    state
        .follow_service
        .unfollow_user(&user.id, &user_id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "User unfollowed successfully"
    })))
}

/// 获取用户的关注者列表
/// GET /api/follows/user/:user_id/followers
async fn get_followers(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
    Query(query): Query<FollowQuery>,
    OptionalAuth(user): OptionalAuth,
) -> Result<Json<Value>> {
    debug!("Getting followers for user: {}", user_id);

    let current_user_id = user.as_ref().map(|u| u.id.as_str());
    let followers = state
        .follow_service
        .get_followers(&user_id, current_user_id, query.page, query.limit)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": followers
    })))
}

/// 获取用户关注的人列表
/// GET /api/follows/user/:user_id/following
async fn get_following(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
    Query(query): Query<FollowQuery>,
    OptionalAuth(user): OptionalAuth,
) -> Result<Json<Value>> {
    debug!("Getting following for user: {}", user_id);

    let current_user_id = user.as_ref().map(|u| u.id.as_str());
    let following = state
        .follow_service
        .get_following(&user_id, current_user_id, query.page, query.limit)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": following
    })))
}

/// 获取用户的关注统计
/// GET /api/follows/user/:user_id/stats
async fn get_follow_stats(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
    OptionalAuth(user): OptionalAuth,
) -> Result<Json<Value>> {
    debug!("Getting follow stats for user: {}", user_id);

    let current_user_id = user.as_ref().map(|u| u.id.as_str());
    let stats = state
        .follow_service
        .get_follow_stats(&user_id, current_user_id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": stats
    })))
}

/// 检查是否关注某用户
/// GET /api/follows/user/:user_id/is-following
async fn check_following(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(target_user_id): Path<String>,
) -> Result<Json<Value>> {
    debug!("Checking if user {} follows user {}", user.id, target_user_id);

    let is_following = state
        .follow_service
        .is_following(&user.id, &target_user_id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": {
            "is_following": is_following
        }
    })))
}

/// 获取共同关注的用户
/// GET /api/follows/mutual/:target_user_id
async fn get_mutual_followers(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(target_user_id): Path<String>,
    Query(query): Query<FollowQuery>,
) -> Result<Json<Value>> {
    debug!("Getting mutual followers between {} and {}", user.id, target_user_id);

    let mutual = state
        .follow_service
        .get_mutual_followers(&user.id, &target_user_id, query.limit)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": mutual
    })))
}