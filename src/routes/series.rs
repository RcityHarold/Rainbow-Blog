use crate::{
    error::{AppError, Result},
    models::series::*,
    services::auth::User,
    state::AppState,
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
        .route("/", get(get_series_list).post(create_series))
        .route("/subscribed", get(get_subscribed_series))
        .route("/:slug", get(get_series).put(update_series).delete(delete_series))
        .route("/:id/articles", post(add_article).delete(remove_article))
        .route("/:id/articles/order", put(update_article_order))
        .route("/:id/subscribe", post(subscribe_series).delete(unsubscribe_series))
}

/// 获取系列列表
/// GET /api/series
async fn get_series_list(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SeriesQuery>,
    OptionalAuth(user): OptionalAuth,
) -> Result<Json<Value>> {
    debug!("Getting series list");

    // 如果未登录用户，只显示公开系列
    let mut final_query = query;
    if user.is_none() && final_query.is_public.is_none() {
        final_query.is_public = Some(true);
    }

    let series_list = state.series_service.get_series_list(final_query).await?;

    Ok(Json(json!({
        "success": true,
        "data": series_list
    })))
}

/// 创建系列
/// POST /api/series
async fn create_series(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Json(request): Json<CreateSeriesRequest>,
) -> Result<Json<Value>> {
    debug!("Creating series: {} for user: {}", request.title, user.id);

    let series = state
        .series_service
        .create_series(&user.id, request)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": series,
        "message": "Series created successfully"
    })))
}

/// 获取系列详情
/// GET /api/series/:slug
async fn get_series(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    OptionalAuth(user): OptionalAuth,
) -> Result<Json<Value>> {
    debug!("Getting series: {}", slug);

    let user_id = user.as_ref().map(|u| u.id.as_str());
    let series = state
        .series_service
        .get_series(&slug, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Series not found".to_string()))?;

    Ok(Json(json!({
        "success": true,
        "data": series
    })))
}

/// 更新系列
/// PUT /api/series/:slug
async fn update_series(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(slug): Path<String>,
    Json(request): Json<UpdateSeriesRequest>,
) -> Result<Json<Value>> {
    debug!("Updating series: {} by user: {}", slug, user.id);

    // 先通过slug获取series_id
    let existing = state
        .series_service
        .get_series(&slug, Some(&user.id))
        .await?
        .ok_or_else(|| AppError::NotFound("Series not found".to_string()))?;

    let updated_series = state
        .series_service
        .update_series(&existing.series.id, &user.id, request)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": updated_series,
        "message": "Series updated successfully"
    })))
}

/// 删除系列
/// DELETE /api/series/:slug
async fn delete_series(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(slug): Path<String>,
) -> Result<Json<Value>> {
    debug!("Deleting series: {} by user: {}", slug, user.id);

    // 先通过slug获取series_id
    let existing = state
        .series_service
        .get_series(&slug, Some(&user.id))
        .await?
        .ok_or_else(|| AppError::NotFound("Series not found".to_string()))?;

    state
        .series_service
        .delete_series(&existing.series.id, &user.id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "Series deleted successfully"
    })))
}

/// 添加文章到系列
/// POST /api/series/:id/articles
async fn add_article(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(series_id): Path<String>,
    Json(request): Json<AddArticleToSeriesRequest>,
) -> Result<Json<Value>> {
    debug!("Adding article to series: {}", series_id);

    let series_article = state
        .series_service
        .add_article_to_series(&series_id, &user.id, request)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": series_article,
        "message": "Article added to series successfully"
    })))
}

/// 从系列中移除文章
/// DELETE /api/series/:id/articles
async fn remove_article(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(series_id): Path<String>,
    Query(params): Query<RemoveArticleParams>,
) -> Result<Json<Value>> {
    debug!("Removing article {} from series: {}", params.article_id, series_id);

    state
        .series_service
        .remove_article_from_series(&series_id, &params.article_id, &user.id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "Article removed from series successfully"
    })))
}

/// 更新文章顺序
/// PUT /api/series/:id/articles/order
async fn update_article_order(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(series_id): Path<String>,
    Json(request): Json<UpdateArticleOrderRequest>,
) -> Result<Json<Value>> {
    debug!("Updating article order for series: {}", series_id);

    state
        .series_service
        .update_article_order(&series_id, &user.id, request)
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "Article order updated successfully"
    })))
}

/// 订阅系列
/// POST /api/series/:id/subscribe
async fn subscribe_series(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(series_id): Path<String>,
) -> Result<Json<Value>> {
    debug!("User {} subscribing to series: {}", user.id, series_id);

    state
        .series_service
        .subscribe_series(&series_id, &user.id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "Series subscribed successfully"
    })))
}

/// 取消订阅系列
/// DELETE /api/series/:id/subscribe
async fn unsubscribe_series(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(series_id): Path<String>,
) -> Result<Json<Value>> {
    debug!("User {} unsubscribing from series: {}", user.id, series_id);

    state
        .series_service
        .unsubscribe_series(&series_id, &user.id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "Series unsubscribed successfully"
    })))
}

/// 获取用户订阅的系列
/// GET /api/series/subscribed
async fn get_subscribed_series(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Query(pagination): Query<PaginationQuery>,
) -> Result<Json<Value>> {
    debug!("Getting subscribed series for user: {}", user.id);

    let page = pagination.page.unwrap_or(1);
    let limit = pagination.limit.unwrap_or(20);

    let series_list = state
        .series_service
        .get_user_subscribed_series(&user.id, page, limit)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": series_list
    })))
}

#[derive(serde::Deserialize)]
struct RemoveArticleParams {
    article_id: String,
}

#[derive(serde::Deserialize)]
struct PaginationQuery {
    page: Option<usize>,
    limit: Option<usize>,
}