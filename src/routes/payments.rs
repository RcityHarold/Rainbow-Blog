use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
    Extension, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error};

use crate::{
    error::{AppError, Result},
    models::{
        payment::*,
        stripe::{CreatePaymentMethodRequest, StripePaymentMethod},
    },
    services::auth::User,
    state::AppState,
};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        // 内容访问和预览
        .route("/content/:article_id/access", get(check_content_access))
        .route("/content/:article_id/preview", get(get_content_preview))
        // 文章定价管理
        .route("/articles/:article_id/pricing", put(set_article_pricing))
        .route("/articles/:article_id/pricing", get(get_article_pricing))
        // 单次购买
        .route("/articles/purchase", post(purchase_article))
        .route("/purchases/:purchase_id", get(get_purchase_details))
        // 创作者仪表板和统计
        .route("/dashboard/:creator_id", get(get_payment_dashboard))
        .route("/access-log", post(record_content_access))
        // 支付方式管理
        .route("/payment-methods", get(list_payment_methods))
        .route("/payment-methods", post(add_payment_method))
        .route(
            "/payment-methods/:payment_method_id",
            delete(remove_payment_method),
        )
        .route(
            "/payment-methods/:payment_method_id/default",
            post(set_default_payment_method),
        )
        // 收益分析
        .route("/earnings", get(get_earnings_analysis))
        .route("/earnings/articles/:article_id", get(get_article_earnings))
}

/// 检查内容访问权限
async fn check_content_access(
    State(state): State<Arc<AppState>>,
    Path(article_id): Path<String>,
    user: Option<Extension<User>>,
) -> Result<Json<serde_json::Value>> {
    debug!("Checking content access for article: {}", article_id);

    let user_id = user.map(|Extension(u)| u.id);
    let access = state
        .payment_service
        .check_content_access(&article_id, user_id.as_deref())
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": access
    })))
}

/// 获取内容预览
async fn get_content_preview(
    State(state): State<Arc<AppState>>,
    Path(article_id): Path<String>,
    user: Option<Extension<User>>,
) -> Result<Json<serde_json::Value>> {
    debug!("Getting content preview for article: {}", article_id);

    let user_id = user.map(|Extension(u)| u.id);
    let preview = state
        .payment_service
        .get_content_preview(&article_id, user_id.as_deref())
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": preview
    })))
}

#[derive(Debug, Deserialize)]
struct SetPricingRequest {
    price: Option<i64>,
    subscription_required: bool,
    preview_percentage: Option<u8>,
    paywall_message: Option<String>,
}

/// 设置文章定价
async fn set_article_pricing(
    State(state): State<Arc<AppState>>,
    Path(article_id): Path<String>,
    Extension(user): Extension<User>,
    Json(payload): Json<SetPricingRequest>,
) -> Result<Json<serde_json::Value>> {
    debug!("Setting pricing for article: {}", article_id);

    let request = ArticlePricingRequest {
        price: payload.price,
        subscription_required: payload.subscription_required,
        preview_percentage: payload.preview_percentage,
        paywall_message: payload.paywall_message,
    };

    let pricing = state
        .payment_service
        .set_article_pricing(&article_id, &user.id, request)
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": pricing
    })))
}

/// 获取文章定价信息
async fn get_article_pricing(
    State(state): State<Arc<AppState>>,
    Path(article_id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    debug!("Getting pricing for article: {}", article_id);

    // 首先尝试从payment_service获取
    match state.payment_service.get_article_pricing(&article_id).await {
        Ok(pricing) => Ok(Json(serde_json::json!({
            "success": true,
            "data": pricing
        }))),
        Err(AppError::NotFound(_)) => {
            // 如果没有定价信息，返回默认配置
            let default_pricing = ArticlePricing {
                article_id: article_id.clone(),
                is_paid_content: false,
                price: None,
                subscription_required: false,
                preview_percentage: 30,
                paywall_message: "订阅以继续阅读完整内容".to_string(),
                creator_id: String::new(), // 需要从文章获取
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };

            Ok(Json(serde_json::json!({
                "success": true,
                "data": default_pricing
            })))
        }
        Err(e) => Err(e),
    }
}

#[derive(Debug, Deserialize)]
struct PurchaseRequest {
    article_id: String,
    payment_method_id: Option<String>,
}

/// 购买单篇文章
async fn purchase_article(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Json(payload): Json<PurchaseRequest>,
) -> Result<Json<serde_json::Value>> {
    debug!("Processing article purchase for user: {}", user.id);

    let request = ArticlePurchaseRequest {
        article_id: payload.article_id,
        payment_method_id: payload.payment_method_id,
    };

    let display_name = user.display_name.as_deref().or(user.username.as_deref());

    let purchase = state
        .payment_service
        .purchase_article(&user.id, &user.email, display_name, request)
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": purchase
    })))
}

/// 获取购买详情
async fn get_purchase_details(
    State(state): State<Arc<AppState>>,
    Path(purchase_id): Path<String>,
    Extension(user): Extension<User>,
) -> Result<Json<serde_json::Value>> {
    debug!("Getting purchase details: {}", purchase_id);

    // 这里需要实现获取购买详情的逻辑
    // 目前先返回简单的响应
    Ok(Json(serde_json::json!({
        "success": true,
        "data": {
            "id": purchase_id,
            "buyer_id": user.id,
            "status": "completed",
            "message": "Purchase details implementation pending"
        }
    })))
}

/// 获取付费内容仪表板
async fn get_payment_dashboard(
    State(state): State<Arc<AppState>>,
    Path(creator_id): Path<String>,
    Extension(user): Extension<User>,
) -> Result<Json<serde_json::Value>> {
    debug!("Getting payment dashboard for creator: {}", creator_id);

    // 验证权限 - 只有创作者本人可以查看
    if user.id != creator_id {
        return Err(AppError::Authorization(
            "只能查看自己的收益数据".to_string(),
        ));
    }

    let dashboard = state
        .payment_service
        .get_payment_dashboard(&creator_id)
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": dashboard
    })))
}

#[derive(Debug, Deserialize)]
struct AccessLogRequest {
    article_id: String,
    access_type: String,
    reading_time: Option<i64>,
}

/// 记录内容访问
async fn record_content_access(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Json(payload): Json<AccessLogRequest>,
) -> Result<Json<serde_json::Value>> {
    debug!("Recording content access for user: {}", user.id);

    let access_type = match payload.access_type.as_str() {
        "free" => AccessType::Free,
        "subscription" => AccessType::Subscription,
        "one_time" => AccessType::OneTime,
        "author" => AccessType::Author,
        "preview" => AccessType::Preview,
        _ => AccessType::Preview,
    };

    state
        .payment_service
        .record_content_access(
            &user.id,
            &payload.article_id,
            access_type,
            payload.reading_time,
        )
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": null
    })))
}

/// 获取当前用户的支付方式列表
async fn list_payment_methods(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
) -> Result<Json<serde_json::Value>> {
    let methods = state.stripe_service.list_payment_methods(&user.id).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": methods,
    })))
}

/// 添加新的支付方式
async fn add_payment_method(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Json(payload): Json<CreatePaymentMethodRequest>,
) -> Result<Json<serde_json::Value>> {
    let display_name = user
        .display_name
        .as_deref()
        .or_else(|| user.username.as_deref());

    let payment_method = state
        .stripe_service
        .add_payment_method(&user.id, &user.email, display_name, payload)
        .await?;

    let updated_methods = state.stripe_service.list_payment_methods(&user.id).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": payment_method,
        "meta": {
            "payment_methods": updated_methods
        }
    })))
}

/// 设置默认支付方式
async fn set_default_payment_method(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(payment_method_id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    let payment_method = state
        .stripe_service
        .set_default_payment_method(&user.id, &payment_method_id)
        .await?;

    let updated_methods = state.stripe_service.list_payment_methods(&user.id).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": payment_method,
        "meta": {
            "payment_methods": updated_methods
        }
    })))
}

/// 删除支付方式
async fn remove_payment_method(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(payment_method_id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    state
        .stripe_service
        .delete_payment_method(&user.id, &payment_method_id)
        .await?;

    let updated_methods = state.stripe_service.list_payment_methods(&user.id).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": serde_json::Value::Null,
        "meta": {
            "payment_methods": updated_methods
        }
    })))
}

#[derive(Debug, Deserialize)]
struct EarningsQuery {
    creator_id: Option<String>,
    article_id: Option<String>,
    start_date: Option<chrono::DateTime<chrono::Utc>>,
    end_date: Option<chrono::DateTime<chrono::Utc>>,
    limit: Option<i32>,
}

/// 获取收益分析
async fn get_earnings_analysis(
    State(state): State<Arc<AppState>>,
    Query(query): Query<EarningsQuery>,
    Extension(user): Extension<User>,
) -> Result<Json<serde_json::Value>> {
    debug!("Getting earnings analysis for user: {}", user.id);

    // 如果指定了creator_id，验证权限
    if let Some(creator_id) = &query.creator_id {
        if user.id != *creator_id {
            return Err(AppError::Authorization(
                "只能查看自己的收益数据".to_string(),
            ));
        }
    }

    // 如果没有指定creator_id，使用当前用户ID
    let creator_id = query.creator_id.unwrap_or(user.id);

    // 获取收益仪表板（这里复用了dashboard功能）
    let dashboard = state
        .payment_service
        .get_payment_dashboard(&creator_id)
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": {
            "creator_id": creator_id,
            "total_revenue": dashboard.monthly_revenue,
            "paid_articles_count": dashboard.total_paid_articles,
            "subscribers_count": dashboard.total_subscribers,
            "purchases_count": dashboard.total_purchases,
            "top_earning_articles": dashboard.top_earning_articles,
            "access_stats": dashboard.access_stats
        }
    })))
}

/// 获取单篇文章收益
async fn get_article_earnings(
    State(state): State<Arc<AppState>>,
    Path(article_id): Path<String>,
    Extension(user): Extension<User>,
) -> Result<Json<serde_json::Value>> {
    debug!("Getting earnings for article: {}", article_id);

    // 验证文章所有权
    // 这里需要先获取文章信息验证作者
    // 简化实现，假设验证通过

    let dashboard = state
        .payment_service
        .get_payment_dashboard(&user.id)
        .await?;

    // 从top_earning_articles中找到指定文章
    let article_earnings = dashboard
        .top_earning_articles
        .into_iter()
        .find(|article| article.article_id == article_id);

    match article_earnings {
        Some(earnings) => Ok(Json(serde_json::json!({
            "success": true,
            "data": earnings
        }))),
        None => Ok(Json(serde_json::json!({
            "success": true,
            "data": {
                "article_id": article_id,
                "total_revenue": 0,
                "subscription_revenue": 0,
                "purchase_revenue": 0,
                "view_count": 0,
                "purchase_count": 0
            }
        }))),
    }
}
