use axum::{
    extract::{Path, Query, State},
    response::Json,
    routing::{delete, get, post, put},
    Extension, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::{
    error::{AppError, Result},
    models::{
        response::{ApiResponse, ErrorResponse},
        subscription::*,
    },
    services::auth::User,
    state::AppState,
};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/plans", post(create_subscription_plan))
        .route("/plans/:plan_id", get(get_subscription_plan))
        .route("/plans/:plan_id", put(update_subscription_plan))
        .route("/plans/:plan_id", delete(deactivate_subscription_plan))
        .route("/creator/:creator_id/plans", get(get_creator_plans))
        .route("/creator/:creator_id/revenue", get(get_creator_revenue))
        .route("/", post(create_subscription))
        .route("/:subscription_id", get(get_subscription))
        .route("/:subscription_id/cancel", post(cancel_subscription))
        .route("/user/:user_id", get(get_user_subscriptions))
        .route("/creator/:creator_id/status", get(get_subscription_status))
        .route("/webhook/stripe", post(handle_stripe_webhook))
}

#[derive(Debug, Deserialize)]
struct CancelSubscriptionPayload {
    at_period_end: Option<bool>,
}

/// 创建订阅计划
async fn create_subscription_plan(
    State(app_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Json(request): Json<CreateSubscriptionPlanRequest>,
) -> Result<Json<ApiResponse<SubscriptionPlan>>> {
    let plan = app_state
        .subscription_service
        .create_subscription_plan(&user.id, request)
        .await?;

    Ok(Json(ApiResponse::success(plan)))
}

/// 获取订阅计划详情
async fn get_subscription_plan(
    State(app_state): State<Arc<AppState>>,
    Path(plan_id): Path<String>,
) -> Result<Json<ApiResponse<SubscriptionPlan>>> {
    let plan = app_state
        .subscription_service
        .get_subscription_plan(&plan_id)
        .await?;

    Ok(Json(ApiResponse::success(plan)))
}

/// 更新订阅计划
async fn update_subscription_plan(
    State(app_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(plan_id): Path<String>,
    Json(request): Json<UpdateSubscriptionPlanRequest>,
) -> Result<Json<ApiResponse<SubscriptionPlan>>> {
    let plan = app_state
        .subscription_service
        .update_subscription_plan(&plan_id, &user.id, request)
        .await?;

    Ok(Json(ApiResponse::success(plan)))
}

/// 停用订阅计划
async fn deactivate_subscription_plan(
    State(app_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(plan_id): Path<String>,
) -> Result<Json<ApiResponse<()>>> {
    let deactivate_request = UpdateSubscriptionPlanRequest {
        name: None,
        description: None,
        price: None,
        benefits: None,
        is_active: Some(false),
    };

    app_state
        .subscription_service
        .update_subscription_plan(&plan_id, &user.id, deactivate_request)
        .await?;

    Ok(Json(ApiResponse::success(())))
}

/// 获取创作者的订阅计划列表
async fn get_creator_plans(
    State(app_state): State<Arc<AppState>>,
    Path(creator_id): Path<String>,
    Query(query): Query<SubscriptionPlanQuery>,
) -> Result<Json<ApiResponse<SubscriptionPlanListResponse>>> {
    let plans = app_state
        .subscription_service
        .get_creator_plans(&creator_id, query)
        .await?;

    Ok(Json(ApiResponse::success(plans)))
}

/// 获取创作者收益统计
async fn get_creator_revenue(
    State(app_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(creator_id): Path<String>,
) -> Result<Json<ApiResponse<CreatorRevenue>>> {
    if user.id != creator_id {
        return Err(AppError::Authorization(
            "无权限查看该创作者收益".to_string(),
        ));
    }

    let revenue = app_state
        .subscription_service
        .get_creator_revenue(&creator_id)
        .await?;

    Ok(Json(ApiResponse::success(revenue)))
}

/// 创建订阅
async fn create_subscription(
    State(app_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Json(request): Json<CreateSubscriptionRequest>,
) -> Result<Json<ApiResponse<SubscriptionDetails>>> {
    let subscription = app_state
        .subscription_service
        .create_subscription(&user.id, request)
        .await?;

    Ok(Json(ApiResponse::success(subscription)))
}

/// 获取订阅详情
async fn get_subscription(
    State(app_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(subscription_id): Path<String>,
) -> Result<Json<ApiResponse<SubscriptionDetails>>> {
    let subscription = app_state
        .subscription_service
        .get_subscription_with_plan(&subscription_id)
        .await?;

    // 只有订阅者本人或创作者可以查看详情
    if subscription.subscriber_id != user.id && subscription.creator.user_id != user.id {
        return Err(AppError::Authorization("无权限查看该订阅详情".to_string()));
    }

    Ok(Json(ApiResponse::success(subscription)))
}

/// 取消订阅
async fn cancel_subscription(
    State(app_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(subscription_id): Path<String>,
    Json(payload): Json<CancelSubscriptionPayload>,
) -> Result<Json<ApiResponse<SubscriptionDetails>>> {
    let subscription = app_state
        .subscription_service
        .cancel_subscription(
            &subscription_id,
            &user.id,
            payload.at_period_end.unwrap_or(true),
        )
        .await?;

    Ok(Json(ApiResponse::success(subscription)))
}

/// 获取用户的订阅列表
async fn get_user_subscriptions(
    State(app_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(user_id): Path<String>,
    Query(query): Query<SubscriptionQuery>,
) -> Result<Json<ApiResponse<SubscriptionListResponse>>> {
    if user.id != user_id {
        return Err(AppError::Authorization(
            "无权限查看该用户订阅列表".to_string(),
        ));
    }

    let subscriptions = app_state
        .subscription_service
        .get_user_subscriptions(&user_id, query)
        .await?;

    Ok(Json(ApiResponse::success(subscriptions)))
}

/// 检查用户对创作者的订阅状态
async fn get_subscription_status(
    State(app_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(creator_id): Path<String>,
) -> Result<Json<ApiResponse<SubscriptionDetails>>> {
    let status = app_state
        .subscription_service
        .check_subscription(&user.id, &creator_id)
        .await?;

    if !status.is_subscribed {
        return Err(AppError::NotFound("尚未订阅该创作者".to_string()));
    }

    let subscription = status
        .subscription
        .ok_or_else(|| AppError::NotFound("未找到订阅详情".to_string()))?;

    Ok(Json(ApiResponse::success(subscription)))
}

/// 处理 Stripe Webhook
async fn handle_stripe_webhook(
    State(app_state): State<Arc<AppState>>,
    Json(event): Json<StripeWebhookEvent>,
) -> Result<Json<ApiResponse<()>>> {
    tracing::info!("Received Stripe webhook: {} ({})", event.r#type, event.id);

    // 这里应该处理各种 Stripe 事件
    // 例如：subscription.updated, subscription.deleted, invoice.payment_succeeded 等
    match event.r#type.as_str() {
        "subscription.updated" => {
            tracing::info!("Processing subscription update webhook");
            // TODO: 处理订阅更新
        }
        "subscription.deleted" => {
            tracing::info!("Processing subscription deletion webhook");
            // TODO: 处理订阅删除
        }
        "invoice.payment_succeeded" => {
            tracing::info!("Processing successful payment webhook");
            // TODO: 处理支付成功
        }
        "invoice.payment_failed" => {
            tracing::info!("Processing failed payment webhook");
            // TODO: 处理支付失败
        }
        _ => {
            tracing::info!("Unhandled webhook type: {}", event.r#type);
        }
    }

    Ok(Json(ApiResponse::success(())))
}
