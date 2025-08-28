use crate::{
    error::{AppError, Result},
    models::article::*,
    services::auth::User,
    state::AppState,
    require_permission,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post, put, delete},
    Router,
    Extension,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{info, debug};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        // 公开路由（不需要认证）
        .route("/", get(list_articles))
        .route("/trending", get(get_trending_articles))
        .route("/popular", get(get_popular_articles))
        .route("/:slug", get(get_article_by_slug))
        
        // 需要认证的路由
        .route("/create", post(create_article))
        .route("/:id/publish", post(publish_article))
        .route("/:id/unpublish", post(unpublish_article))
        .route("/:id", put(update_article).delete(delete_article))
        .route("/:id/view", post(increment_view_count))
}

/// 获取文章列表
/// GET /api/articles
pub async fn list_articles(
    State(app_state): State<Arc<AppState>>,
    Query(query): Query<ArticleQuery>,
    user: Option<Extension<User>>,
) -> Result<Json<Value>> {
    debug!("Fetching articles list with query: {:?}", query);

    let result = app_state.article_service.get_articles(query).await?;

    // 如果用户已登录，可以添加额外信息（如是否收藏等）
    let user_id = user.as_ref().map(|u| &u.0.id);

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

/// 获取热门文章
/// GET /api/articles/trending
pub async fn get_trending_articles(
    State(app_state): State<Arc<AppState>>,
    Query(query): Query<ArticleQuery>,
) -> Result<Json<Value>> {
    debug!("Fetching trending articles");

    let mut trending_query = query;
    trending_query.sort = Some("trending".to_string());
    trending_query.limit = trending_query.limit.or(Some(10));

    let result = app_state.article_service.get_articles(trending_query).await?;

    Ok(Json(json!({
        "success": true,
        "data": result.data
    })))
}

/// 获取热门文章
/// GET /api/articles/popular
pub async fn get_popular_articles(
    State(app_state): State<Arc<AppState>>,
    Query(query): Query<ArticleQuery>,
) -> Result<Json<Value>> {
    debug!("Fetching popular articles");

    let mut popular_query = query;
    popular_query.sort = Some("popular".to_string());
    popular_query.limit = popular_query.limit.or(Some(10));

    let result = app_state.article_service.get_articles(popular_query).await?;

    Ok(Json(json!({
        "success": true,
        "data": result.data
    })))
}

/// 根据 slug 获取文章详情
/// GET /api/articles/:slug
pub async fn get_article_by_slug(
    State(app_state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    user: Option<Extension<User>>,
) -> Result<Json<Value>> {
    debug!("Fetching article by slug: {}", slug);

    // 获取当前用户ID（如果已登录）
    let user_id = user.as_ref().map(|u| u.0.id.as_str());

    // 获取文章完整信息
    let article_response = app_state.article_service
        .get_article_with_details(&slug, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Article not found".to_string()))?;

    // 检查文章可见性
    if !article_response.status.can_be_viewed_by_public() {
        // 只有作者本人可以查看未发布的文章
        if user_id != Some(&article_response.author.id) {
            return Err(AppError::NotFound("Article not found".to_string()));
        }
    }

    // 异步增加浏览次数（不阻塞响应）
    let article_service = app_state.article_service.clone();
    let article_id = article_response.id.clone();
    tokio::spawn(async move {
        if let Err(e) = article_service.increment_view_count(&article_id).await {
            tracing::warn!("Failed to increment view count for article {}: {}", article_id, e);
        }
    });

    Ok(Json(json!({
        "success": true,
        "data": article_response
    })))
}

/// 创建新文章
/// POST /api/articles/create
pub async fn create_article(
    State(app_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Json(request): Json<CreateArticleRequest>,
) -> Result<Json<Value>> {
    debug!("Creating article for user: {}", user.id);

    // 检查邮箱验证状态
    if !user.is_verified {
        return Err(AppError::Authorization("创建文章需要验证邮箱，请前往 Rainbow-Auth 完成邮箱验证".to_string()));
    }

    // 检查权限
    require_permission!(app_state.auth_service, user, "article.create");

    // 创建文章
    let article = app_state.article_service.create_article(&user.id, request).await?;

    info!("Created article: {} by user: {}", article.id, user.id);

    Ok(Json(json!({
        "success": true,
        "data": article,
        "message": "Article created successfully"
    })))
}

/// 更新文章
/// PUT /api/articles/:id
pub async fn update_article(
    State(app_state): State<Arc<AppState>>,
    Path(article_id): Path<String>,
    Extension(user): Extension<User>,
    Json(request): Json<UpdateArticleRequest>,
) -> Result<Json<Value>> {
    debug!("Updating article: {} by user: {}", article_id, user.id);

    // 检查权限
    require_permission!(app_state.auth_service, user, "article.update");

    // 更新文章
    let article = app_state.article_service.update_article(&article_id, &user.id, request).await?;

    info!("Updated article: {} by user: {}", article_id, user.id);

    Ok(Json(json!({
        "success": true,
        "data": article,
        "message": "Article updated successfully"
    })))
}

/// 发布文章
/// POST /api/articles/:id/publish
pub async fn publish_article(
    State(app_state): State<Arc<AppState>>,
    Path(article_id): Path<String>,
    Extension(user): Extension<User>,
) -> Result<Json<Value>> {
    debug!("Publishing article: {} by user: {}", article_id, user.id);

    // 检查邮箱验证状态
    if !user.is_verified {
        return Err(AppError::Authorization("发布文章需要验证邮箱，请前往 Rainbow-Auth 完成邮箱验证".to_string()));
    }

    // 检查权限
    require_permission!(app_state.auth_service, user, "article.update");

    // 发布文章
    let article = app_state.article_service.publish_article(&article_id, &user.id).await?;

    info!("Published article: {} by user: {}", article_id, user.id);

    Ok(Json(json!({
        "success": true,
        "data": article,
        "message": "Article published successfully"
    })))
}

/// 取消发布文章
/// POST /api/articles/:id/unpublish
pub async fn unpublish_article(
    State(app_state): State<Arc<AppState>>,
    Path(article_id): Path<String>,
    Extension(user): Extension<User>,
) -> Result<Json<Value>> {
    debug!("Unpublishing article: {} by user: {}", article_id, user.id);

    // 检查权限
    require_permission!(app_state.auth_service, user, "article.update");

    // 取消发布文章
    let article = app_state.article_service.unpublish_article(&article_id, &user.id).await?;

    info!("Unpublished article: {} by user: {}", article_id, user.id);

    Ok(Json(json!({
        "success": true,
        "data": article,
        "message": "Article unpublished successfully"
    })))
}

/// 删除文章
/// DELETE /api/articles/:id
pub async fn delete_article(
    State(app_state): State<Arc<AppState>>,
    Path(article_id): Path<String>,
    Extension(user): Extension<User>,
) -> Result<Json<Value>> {
    debug!("Deleting article: {} by user: {}", article_id, user.id);

    // 检查权限
    require_permission!(app_state.auth_service, user, "article.delete");

    // 删除文章
    app_state.article_service.delete_article(&article_id, &user.id).await?;

    info!("Deleted article: {} by user: {}", article_id, user.id);

    Ok(Json(json!({
        "success": true,
        "message": "Article deleted successfully"
    })))
}

/// 增加文章浏览次数
/// POST /api/articles/:id/view
pub async fn increment_view_count(
    State(app_state): State<Arc<AppState>>,
    Path(article_id): Path<String>,
) -> Result<Json<Value>> {
    debug!("Incrementing view count for article: {}", article_id);

    // 检查文章是否存在
    let article = app_state.article_service.get_article_by_id(&article_id).await?
        .ok_or_else(|| AppError::NotFound("Article not found".to_string()))?;

    // 只有已发布的文章才能增加浏览次数
    if !article.is_published() {
        return Err(AppError::BadRequest("Cannot increment view count for unpublished article".to_string()));
    }

    // 增加浏览次数
    app_state.article_service.increment_view_count(&article_id).await?;

    Ok(Json(json!({
        "success": true,
        "message": "View count incremented"
    })))
}