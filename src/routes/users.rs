use crate::{
    error::{AppError, Result},
    models::user::*,
    services::auth::User,
    state::AppState,
    require_permission,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post, put},
    Router,
    Extension,
};
use serde_json::{json, Value};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, debug};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        // 公开路由
        .route("/", get(list_users))
        .route("/popular", get(get_popular_users))
        .route("/search", get(search_users))
        .route("/:username", get(get_user_profile))
        .route("/:username/articles", get(get_user_articles))
        .route("/:username/stats", get(get_user_activity_stats))
        
        // 需要认证的路由
        .route("/me", get(get_current_user_profile))
        .route("/me", put(update_current_user_profile))
        .route("/me/articles", get(get_current_user_articles))
}

#[derive(Debug, Deserialize)]
pub struct UserListQuery {
    pub page: Option<usize>,
    pub limit: Option<usize>,
    pub search: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UserArticlesQuery {
    pub page: Option<usize>,
    pub limit: Option<usize>,
    pub status: Option<String>,
}

/// 获取用户列表
/// GET /api/users
pub async fn list_users(
    State(app_state): State<Arc<AppState>>,
    Query(query): Query<UserListQuery>,
) -> Result<Json<Value>> {
    debug!("Fetching users list with query: {:?}", query);

    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20).min(100); // 限制最大每页数量

    let result = app_state.user_service.get_users(page, limit, query.search).await?;

    Ok(Json(json!({
        "success": true,
        "data": {
            "users": result.data,
            "pagination": {
                "current_page": result.page,
                "total_pages": result.total_pages,
                "total_items": result.total,
                "items_per_page": result.per_page,
                "has_next": result.page < result.total_pages,
                "has_prev": result.page > 1,
            }
        }
    })))
}

/// 获取热门用户
/// GET /api/users/popular
pub async fn get_popular_users(
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<Value>> {
    debug!("Fetching popular users");

    let users = app_state.user_service.get_popular_users(20).await?;

    Ok(Json(json!({
        "success": true,
        "data": users
    })))
}

/// 搜索用户
/// GET /api/users/search
pub async fn search_users(
    State(app_state): State<Arc<AppState>>,
    Query(query): Query<serde_json::Value>,
) -> Result<Json<Value>> {
    let search_query = query.get("q")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Search query parameter 'q' is required".to_string()))?;

    let limit = query.get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(20)
        .min(100);

    debug!("Searching users with query: {}", search_query);

    let users = app_state.user_service.search_users(search_query, limit).await?;

    Ok(Json(json!({
        "success": true,
        "data": users
    })))
}

/// 根据用户名获取用户资料
/// GET /api/users/:username
pub async fn get_user_profile(
    State(app_state): State<Arc<AppState>>,
    Path(username): Path<String>,
) -> Result<Json<Value>> {
    debug!("Fetching user profile for username: {}", username);

    let profile = app_state.user_service.get_profile_by_username(&username).await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    // 检查用户是否被暂停
    if profile.is_suspended {
        return Err(AppError::NotFound("User not found".to_string()));
    }

    Ok(Json(json!({
        "success": true,
        "data": profile.to_response()
    })))
}

/// 获取用户的文章列表
/// GET /api/users/:username/articles
pub async fn get_user_articles(
    State(app_state): State<Arc<AppState>>,
    Path(username): Path<String>,
    Query(query): Query<UserArticlesQuery>,
) -> Result<Json<Value>> {
    debug!("Fetching articles for username: {}", username);

    // 获取用户资料
    let profile = app_state.user_service.get_profile_by_username(&username).await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20).min(100);

    // 默认只显示已发布的文章
    let result = app_state.article_service.get_user_articles(
        &profile.user_id,
        page,
        limit,
        false, // include_drafts = false for public access
    ).await?;

    Ok(Json(json!({
        "success": true,
        "data": {
            "articles": result.data,
            "user": profile.to_response(),
            "pagination": {
                "current_page": result.page,
                "total_pages": result.total_pages,
                "total_items": result.total,
                "items_per_page": result.per_page,
                "has_next": result.page < result.total_pages,
                "has_prev": result.page > 1,
            }
        }
    })))
}

/// 获取用户活动统计
/// GET /api/users/:username/stats
pub async fn get_user_activity_stats(
    State(app_state): State<Arc<AppState>>,
    Path(username): Path<String>,
) -> Result<Json<Value>> {
    debug!("Fetching activity stats for username: {}", username);

    // 获取用户资料
    let profile = app_state.user_service.get_profile_by_username(&username).await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let stats = app_state.user_service.get_user_stats(&profile.user_id).await?;

    Ok(Json(json!({
        "success": true,
        "data": {
            "user": profile.to_response(),
            "activity": stats
        }
    })))
}

/// 获取当前用户资料
/// GET /api/users/me
pub async fn get_current_user_profile(
    State(app_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
) -> Result<Json<Value>> {
    debug!("Fetching current user profile for user: {}", user.id);

    let profile = app_state.user_service.get_or_create_profile(
        &user.id,
        &user.email,
        user.is_verified,
        user.username,
        user.display_name,
    ).await?;

    let stats = app_state.user_service.get_user_stats(&user.id).await?;

    Ok(Json(json!({
        "success": true,
        "data": {
            "profile": profile.to_response(),
            "auth_info": {
                "id": user.id,
                "email": user.email,
                "is_verified": user.is_verified,
                "created_at": user.created_at,
                "roles": user.roles,
                "permissions": user.permissions,
            },
            "activity": stats
        }
    })))
}

/// 更新当前用户资料
/// PUT /api/users/me
pub async fn update_current_user_profile(
    State(app_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Json(request): Json<UpdateUserProfileRequest>,
) -> Result<Json<Value>> {
    debug!("Updating current user profile for user: {}", user.id);

    // 检查权限
    require_permission!(app_state.auth_service, user, "user.update_profile");

    // 更新用户资料
    let profile = app_state.user_service.update_profile(&user.id, request).await?;

    info!("Updated user profile for user: {}", user.id);

    Ok(Json(json!({
        "success": true,
        "data": profile.to_response(),
        "message": "Profile updated successfully"
    })))
}

/// 获取当前用户的文章列表（包括草稿）
/// GET /api/users/me/articles
pub async fn get_current_user_articles(
    State(app_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Query(query): Query<UserArticlesQuery>,
) -> Result<Json<Value>> {
    debug!("Fetching articles for current user: {}", user.id);

    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20).min(100);

    // 用户可以查看自己的所有文章，包括草稿
    let include_drafts = match query.status.as_deref() {
        Some("draft") => true,
        Some("published") => false,
        _ => true, // 默认包括所有状态
    };

    let result = app_state.article_service.get_user_articles(
        &user.id,
        page,
        limit,
        include_drafts,
    ).await?;

    Ok(Json(json!({
        "success": true,
        "data": {
            "articles": result.data,
            "pagination": {
                "current_page": result.page,
                "total_pages": result.total_pages,
                "total_items": result.total,
                "items_per_page": result.per_page,
                "has_next": result.page < result.total_pages,
                "has_prev": result.page > 1,
            }
        }
    })))
}