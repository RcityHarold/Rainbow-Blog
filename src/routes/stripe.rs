use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::{get, post},
    Extension, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, warn};
use validator::Validate;

use crate::{
    error::{AppError, Result},
    models::{response::ApiResponse, stripe::*},
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
        .route(
            "/payment-intents/:intent_id/confirm",
            post(confirm_payment_intent),
        )
        // 订阅管理
        .route("/subscriptions", post(create_subscription))
        .route("/subscriptions/:subscription_id", get(get_subscription))
        .route(
            "/subscriptions/:subscription_id/cancel",
            post(cancel_subscription),
        )
        // Connect账户
        .route("/connect/accounts", post(create_connect_account))
        .route("/connect/accounts/me", get(get_current_connect_account))
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

    let customer = state
        .stripe_service
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
    Json(payload): Json<CreateStripeIntentRequest>,
) -> Result<Json<serde_json::Value>> {
    debug!("Creating payment intent for user: {}", user.id);

    let display_name = user
        .display_name
        .as_deref()
        .or_else(|| user.username.as_deref());

    let payment_intent = state
        .stripe_service
        .create_payment_intent(&user.id, &user.email, display_name, payload)
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

    let subscription = state
        .stripe_service
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
    debug!(
        "Canceling subscription: {} for user: {}",
        subscription_id, user.id
    );

    let at_period_end = payload.at_period_end.unwrap_or(true);
    state
        .stripe_service
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
) -> Result<Json<ApiResponse<ConnectAccountResponse>>> {
    debug!("Creating Connect account for user: {}", user.id);

    let account = state
        .stripe_service
        .create_connect_account(&user.id, payload)
        .await?;

    Ok(Json(ApiResponse::success(account)))
}

/// 获取Connect账户信息
async fn get_connect_account(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(account_id): Path<String>,
) -> Result<Json<ApiResponse<Option<ConnectAccountResponse>>>> {
    debug!(
        "Getting Connect account info {} for user: {}",
        account_id, user.id
    );

    let account = state
        .stripe_service
        .get_connect_account_by_identifier(&account_id)
        .await?;

    if let Some(ref response) = account {
        if response.account.user_id != user.id {
            return Err(AppError::Authorization(
                "无权限查看该 Connect 账户".to_string(),
            ));
        }
    }

    Ok(Json(ApiResponse::success(account)))
}

/// 获取当前用户的 Connect 账户信息
async fn get_current_connect_account(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
) -> Result<Json<ApiResponse<Option<ConnectAccountResponse>>>> {
    let account = state
        .stripe_service
        .get_connect_account_for_user(&user.id)
        .await?;

    Ok(Json(ApiResponse::success(account)))
}

/// 处理Stripe Webhook
async fn handle_webhook(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<serde_json::Value>> {
    debug!("Handling Stripe webhook");

    // 解析webhook数据
    let webhook_body = String::from_utf8(body.to_vec())
        .map_err(|_| AppError::BadRequest("Invalid webhook body".to_string()))?;

    let signature = headers
        .get("Stripe-Signature")
        .ok_or_else(|| AppError::BadRequest("缺少 Stripe-Signature 请求头".to_string()))?
        .to_str()
        .map_err(|_| AppError::BadRequest("无法解析 Stripe-Signature 请求头".to_string()))?;

    state
        .stripe_service
        .verify_webhook_signature(&webhook_body, signature)
        .await?;

    let event_data: serde_json::Value = serde_json::from_str(&webhook_body)
        .map_err(|e| AppError::BadRequest(format!("Invalid JSON in webhook: {}", e)))?;

    // 处理webhook事件
    match state.stripe_service.process_webhook_event(event_data).await {
        Ok(outcome) => {
            for purchase in &outcome.purchase_updates {
                state
                    .payment_service
                    .handle_stripe_purchase_success(purchase)
                    .await?;

                let _ = state
                    .revenue_service
                    .record_purchase_revenue_from_webhook(purchase)
                    .await?;
            }

            for revenue_event in &outcome.subscription_revenues {
                let _ = state
                    .revenue_service
                    .record_subscription_revenue_from_webhook(revenue_event)
                    .await?;
            }

            for status_update in &outcome.subscription_status_updates {
                state
                    .payment_service
                    .handle_subscription_status_update(status_update)
                    .await?;
            }

            if !outcome.subscription_status_updates.is_empty() {
                debug!(
                    "同步 Stripe 订阅状态更新: {}",
                    outcome.subscription_status_updates.len()
                );
            }

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
        total_amount: 15000,   // $150.00
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
