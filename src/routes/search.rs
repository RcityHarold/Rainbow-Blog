use crate::{
    error::Result,
    models::search::*,
    state::AppState,
    utils::middleware::OptionalAuth,
};
use axum::{
    extract::{Query, State},
    response::Json,
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::debug;

#[derive(Debug, Deserialize)]
pub struct SuggestQuery {
    pub q: String,
    pub limit: Option<i32>,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(search))
        .route("/advanced", post(advanced_search))
        .route("/suggestions", get(get_suggestions))
}

/// 全局搜索
/// GET /api/search?q=query&type=all&page=1&limit=10
async fn search(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Value>> {
    debug!("Performing search with query: {:?}", query);

    let results = state.search_service.search(query).await?;

    Ok(Json(json!({
        "success": true,
        "data": results
    })))
}

/// 高级搜索
/// POST /api/search/advanced
/// Body: AdvancedSearchQuery
async fn advanced_search(
    State(state): State<Arc<AppState>>,
    OptionalAuth(user): OptionalAuth,
    Json(query): Json<AdvancedSearchQuery>,
) -> Result<Json<Value>> {
    debug!("Performing advanced search with query: {:?}", query);
    
    let user_id = user.as_ref().map(|u| u.id.as_str());
    let results = state.search_service.advanced_search(user_id, query).await?;
    
    Ok(Json(json!({
        "success": true,
        "data": results
    })))
}

/// 获取搜索建议
/// GET /api/search/suggestions?q=query&limit=10
async fn get_suggestions(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SuggestQuery>,
) -> Result<Json<Value>> {
    debug!("Getting search suggestions for: {}", query.q);

    let suggestions = state
        .search_service
        .get_search_suggestions(&query.q, query.limit)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": suggestions
    })))
}