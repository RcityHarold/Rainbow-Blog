use crate::{
    error::Result,
    models::analytics::*,
    state::AppState,
    services::auth::User,
};
use axum::{
    extract::{Query, State},
    response::Json,
    routing::{get, post},
    Extension, Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::debug;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/dashboard", get(get_dashboard))
        .route("/overview", get(get_overview))
        .route("/articles", get(get_article_analytics))
        .route("/audience", get(get_audience))
        .route("/tags", get(get_tag_analytics))
        .route("/trends", get(get_trends))
        .route("/realtime", get(get_realtime))
        .route("/export", post(export_data))
}

/// 获取完整的分析仪表板
/// GET /api/stats/dashboard
async fn get_dashboard(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Query(query): Query<AnalyticsQuery>,
) -> Result<Json<Value>> {
    debug!("Getting analytics dashboard for user: {}", user.id);

    let dashboard = state
        .analytics_service
        .get_user_dashboard(&user.id, query)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": dashboard
    })))
}

/// 获取用户统计概览
/// GET /api/stats/overview
async fn get_overview(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
) -> Result<Json<Value>> {
    debug!("Getting analytics overview for user: {}", user.id);

    let overview = state
        .analytics_service
        .get_user_overview(&user.id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": overview
    })))
}

/// 获取文章分析数据
/// GET /api/stats/articles?limit=10
async fn get_article_analytics(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Query(params): Query<ArticleAnalyticsQuery>,
) -> Result<Json<Value>> {
    debug!("Getting article analytics for user: {}", user.id);

    let limit = params.limit.unwrap_or(10);
    let articles = state
        .analytics_service
        .get_recent_article_analytics(&user.id, limit)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": articles
    })))
}

/// 获取受众分析
/// GET /api/stats/audience
async fn get_audience(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Query(query): Query<AnalyticsQuery>,
) -> Result<Json<Value>> {
    debug!("Getting audience analytics for user: {}", user.id);

    let end_date = query.end_date.unwrap_or_else(chrono::Utc::now);
    let start_date = query.start_date.unwrap_or(end_date - chrono::Duration::days(30));

    let audience = state
        .analytics_service
        .get_audience_analytics(&user.id, &start_date, &end_date)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": audience
    })))
}

/// 获取标签分析
/// GET /api/stats/tags?limit=10
async fn get_tag_analytics(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Query(params): Query<TagAnalyticsQuery>,
) -> Result<Json<Value>> {
    debug!("Getting tag analytics for user: {}", user.id);

    let limit = params.limit.unwrap_or(10);
    let tags = state
        .analytics_service
        .get_top_tags_analytics(&user.id, limit)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": tags
    })))
}

/// 获取趋势分析
/// GET /api/stats/trends
async fn get_trends(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Query(query): Query<AnalyticsQuery>,
) -> Result<Json<Value>> {
    debug!("Getting trend analytics for user: {}", user.id);

    let end_date = query.end_date.unwrap_or_else(chrono::Utc::now);
    let start_date = query.start_date.unwrap_or(end_date - chrono::Duration::days(30));

    let trends = state
        .analytics_service
        .get_trend_analytics(&user.id, &start_date, &end_date)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": trends
    })))
}

/// 获取实时分析
/// GET /api/stats/realtime
async fn get_realtime(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
) -> Result<Json<Value>> {
    debug!("Getting realtime analytics for user: {}", user.id);

    let realtime = state
        .analytics_service
        .get_realtime_analytics(&user.id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": realtime
    })))
}

/// 导出分析数据
/// POST /api/stats/export
/// Body: ExportOptions
async fn export_data(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Json(options): Json<ExportOptions>,
) -> Result<Json<Value>> {
    debug!("Exporting analytics data for user: {} with format: {:?}", user.id, options.format);

    let data = state
        .analytics_service
        .export_analytics(&user.id, options)
        .await?;

    // 对于JSON和CSV，我们返回base64编码的数据
    let base64_data = base64::encode(&data);

    Ok(Json(json!({
        "success": true,
        "data": {
            "content": base64_data,
            "size": data.len()
        },
        "message": "Export completed successfully"
    })))
}

// Query parameter structs
#[derive(serde::Deserialize)]
struct ArticleAnalyticsQuery {
    limit: Option<i32>,
}

#[derive(serde::Deserialize)]
struct TagAnalyticsQuery {
    limit: Option<i32>,
}