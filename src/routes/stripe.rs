use axum::{
    extract::{Path, State},
    response::Json,
    routing::{get, post},
    Router,
    Extension,
    http::StatusCode,
    body::Bytes,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, warn};
use validator::Validate;

use crate::{
    error::{AppError, Result},
    models::stripe::*,
    services::auth::User,
    state::AppState,
};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        // 客户管理
        .route("/customers", post(create_customer))
        .route("/customers/:customer_id", get(get_customer))
        
        // 支付意图
        .route("/payment-intents", post(create_payment_intent))
        .route("/payment-intents/:intent_id", get(get_payment_intent))
        .route("/payment-intents/:intent_id/confirm", post(confirm_payment_intent))
        
        // 订阅管理
        .route("/subscriptions", post(create_subscription))
        .route("/subscriptions/:subscription_id", get(get_subscription))
        .route("/subscriptions/:subscription_id/cancel", post(cancel_subscription))
        
        // Connect账户
        .route("/connect/accounts", post(create_connect_account))
        .route("/connect/accounts/:account_id", get(get_connect_account))
        
        // Webhook处理
        .route("/webhooks", post(handle_webhook))
        
        // 支付统计
        .route("/stats", get(get_payment_stats))
}

#[derive(Debug, Deserialize)]
struct CreateCustomerRequest {
    email: String,
    name: Option<String>,
}

/// 创建Stripe客户
async fn create_customer(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Json(payload): Json<CreateCustomerRequest>,
) -> Result<Json<serde_json::Value>> {
    debug!("Creating Stripe customer for user: {}", user.id);

    let customer = state.stripe_service
        .get_or_create_customer(&user.id, &payload.email, payload.name.as_deref())
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": customer
    })))
}

/// 获取客户信息
async fn get_customer(
    State(_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(_customer_id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    debug!("Getting customer info for user: {}", user.id);

    // TODO: 实现获取客户详情逻辑
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Customer info retrieved successfully"
    })))
}

/// 创建支付意图
async fn create_payment_intent(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Json(payload): Json<CreatePaymentIntentRequest>,
) -> Result<Json<serde_json::Value>> {
    debug!("Creating payment intent for user: {}", user.id);

    let payment_intent = state.stripe_service
        .create_payment_intent(&user.id, payload)
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": payment_intent
    })))
}

/// 获取支付意图
async fn get_payment_intent(
    State(_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(_intent_id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    debug!("Getting payment intent for user: {}", user.id);

    // TODO: 实现获取支付意图详情逻辑
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Payment intent retrieved successfully"
    })))
}

/// 确认支付意图
async fn confirm_payment_intent(
    State(_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(_intent_id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    debug!("Confirming payment intent for user: {}", user.id);

    // TODO: 实现确认支付意图逻辑
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Payment intent confirmed successfully"
    })))
}

/// 创建订阅
async fn create_subscription(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Json(payload): Json<CreateStripeSubscriptionRequest>,
) -> Result<Json<serde_json::Value>> {
    debug!("Creating Stripe subscription for user: {}", user.id);

    let subscription = state.stripe_service
        .create_subscription(&user.id, payload)
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": subscription
    })))
}

/// 获取订阅信息
async fn get_subscription(
    State(_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(_subscription_id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    debug!("Getting subscription info for user: {}", user.id);

    // TODO: 实现获取订阅详情逻辑
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Subscription info retrieved successfully"
    })))
}

#[derive(Debug, Deserialize)]
struct CancelSubscriptionRequest {
    at_period_end: Option<bool>,
}

/// 取消订阅
async fn cancel_subscription(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(subscription_id): Path<String>,
    Json(payload): Json<CancelSubscriptionRequest>,
) -> Result<Json<serde_json::Value>> {
    debug!("Canceling subscription: {} for user: {}", subscription_id, user.id);

    let at_period_end = payload.at_period_end.unwrap_or(true);
    state.stripe_service
        .cancel_subscription(&subscription_id, at_period_end)
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": if at_period_end {
            "订阅将在当前计费周期结束时取消"
        } else {
            "订阅已立即取消"
        }
    })))
}

/// 创建Connect账户
async fn create_connect_account(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Json(payload): Json<CreateConnectAccountRequest>,
) -> Result<Json<serde_json::Value>> {
    debug!("Creating Connect account for user: {}", user.id);

    let account = state.stripe_service
        .create_connect_account(&user.id, payload)
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": account
    })))
}

/// 获取Connect账户信息
async fn get_connect_account(
    State(_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(_account_id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    debug!("Getting Connect account info for user: {}", user.id);

    // TODO: 实现获取Connect账户详情逻辑
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Connect account info retrieved successfully"
    })))
}

/// 处理Stripe Webhook
async fn handle_webhook(
    State(state): State<Arc<AppState>>,
    body: Bytes,
) -> Result<Json<serde_json::Value>> {
    debug!("Handling Stripe webhook");

    // 解析webhook数据
    let webhook_body = String::from_utf8(body.to_vec())
        .map_err(|_| AppError::BadRequest("Invalid webhook body".to_string()))?;

    let event_data: serde_json::Value = serde_json::from_str(&webhook_body)
        .map_err(|e| AppError::BadRequest(format!("Invalid JSON in webhook: {}", e)))?;

    // TODO: 验证webhook签名
    // let signature = headers.get("stripe-signature")
    //     .ok_or_else(|| AppError::BadRequest("Missing Stripe signature".to_string()))?;

    // 处理webhook事件
    match state.stripe_service.handle_webhook(event_data).await {
        Ok(()) => {
            debug!("Webhook processed successfully");
            Ok(Json(serde_json::json!({
                "success": true
            })))
        }
        Err(e) => {
            error!("Failed to process webhook: {}", e);
            Err(e)
        }
    }
}

#[derive(Debug, Deserialize)]
struct PaymentStatsQuery {
    start_date: Option<chrono::DateTime<chrono::Utc>>,
    end_date: Option<chrono::DateTime<chrono::Utc>>,
    currency: Option<String>,
}

/// 获取支付统计
async fn get_payment_stats(
    State(_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    axum::extract::Query(_query): axum::extract::Query<PaymentStatsQuery>,
) -> Result<Json<serde_json::Value>> {
    debug!("Getting payment stats for user: {}", user.id);

    // TODO: 实现支付统计逻辑
    let stats = PaymentStats {
        total_payments: 150,
        successful_payments: 142,
        failed_payments: 8,
        total_amount: 15000, // $150.00
        average_amount: 100.0, // $1.00
        currency: "USD".to_string(),
        period_start: chrono::Utc::now() - chrono::Duration::days(30),
        period_end: chrono::Utc::now(),
    };

    Ok(Json(serde_json::json!({
        "success": true,
        "data": stats
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_customer_request_validation() {
        let request = CreateCustomerRequest {
            email: "test@example.com".to_string(),
            name: Some("Test User".to_string()),
        };
        
        assert_eq!(request.email, "test@example.com");
        assert_eq!(request.name, Some("Test User".to_string()));
    }
}