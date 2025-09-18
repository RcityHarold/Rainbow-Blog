use crate::{
    error::{AppError, Result},
    state::AppState,
};
use axum::{routing::get, extract::State, response::Json, Router};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::debug;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/", get(diagnostics))
}

/// 诊断端点（仅开发环境可用）
/// GET /api/blog/diagnostics
async fn diagnostics(State(state): State<Arc<AppState>>) -> Result<Json<Value>> {
    if !state.is_development() {
        return Err(AppError::forbidden("Diagnostics endpoint is only available in development"));
    }

    debug!("Running diagnostics endpoint");

    // 基本配置信息
    let ns = state.config.database_namespace.clone();
    let db = state.config.database_name.clone();
    let url = state.config.database_url.clone();

    // 统计若干关键表计数
    async fn count_table(state: &AppState, table: &str) -> usize {
        let sql = format!("SELECT count() AS total FROM {}", table);
        match state.db.query(&sql).await {
            Ok(mut resp) => {
                if let Ok(Some(v)) = resp.take::<Option<Value>>(0) {
                    v.get("total").and_then(|x| x.as_i64()).unwrap_or(0) as usize
                } else { 0 }
            }
            Err(_) => 0,
        }
    }

    let tag_count = count_table(&state, "tag").await;
    let article_tag_count = count_table(&state, "article_tag").await;
    let article_count = count_table(&state, "article").await;
    let publication_count = count_table(&state, "publication").await;

    // 抽样部分标签（原始返回，避免反序列化问题）
    let sample_tags: Vec<Value> = match state
        .db
        .query("SELECT id, name, slug FROM tag ORDER BY created_at DESC LIMIT 5")
        .await
    {
        Ok(mut resp) => resp.take(0).unwrap_or_default(),
        Err(_) => Vec::new(),
    };

    Ok(Json(json!({
        "success": true,
        "data": {
            "database": {
                "namespace": ns,
                "name": db,
                "url": url,
            },
            "counts": {
                "tag": tag_count,
                "article_tag": article_tag_count,
                "article": article_count,
                "publication": publication_count,
            },
            "samples": {
                "tags": sample_tags,
            }
        }
    })))
}

