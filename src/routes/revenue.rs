use axum::{
    extract::{Path, Query, State},
    response::Json,
    routing::{get, post},
    Router,
    Extension,
};
use serde::{Deserialize, Serialize};
use chrono::Datelike;
use std::sync::Arc;
use tracing::{debug, error};

use crate::{
    error::{AppError, Result},
    models::revenue::*,
    services::auth::User,
    state::AppState,
};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        // 收益仪表板
        .route("/dashboard", get(get_revenue_dashboard))
        
        // 收益统计
        .route("/stats", get(get_revenue_stats))
        .route("/transactions", get(get_revenue_transactions))
        
        // 支付管理
        .route("/payouts", post(create_payout))
        .route("/payouts", get(get_payouts))
        .route("/payouts/:payout_id", get(get_payout_details))
        
        // 银行账户管理
        .route("/bank-accounts", get(get_bank_accounts))
        .route("/bank-accounts", post(add_bank_account))
        .route("/bank-accounts/:account_id/verify", post(verify_bank_account))
        .route("/bank-accounts/:account_id/default", post(set_default_bank_account))
        
        // 收益设置
        .route("/settings", get(get_revenue_settings))
        .route("/settings", post(update_revenue_settings))
}

/// 获取收益仪表板
async fn get_revenue_dashboard(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
) -> Result<Json<serde_json::Value>> {
    debug!("Getting revenue dashboard for user: {}", user.id);

    let dashboard = state.revenue_service
        .get_revenue_dashboard(&user.id)
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": dashboard
    })))
}

#[derive(Debug, Deserialize)]
struct RevenueStatsQuery {
    period: Option<String>,
    start_date: Option<chrono::DateTime<chrono::Utc>>,
    end_date: Option<chrono::DateTime<chrono::Utc>>,
}

/// 获取收益统计
async fn get_revenue_stats(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RevenueStatsQuery>,
    Extension(user): Extension<User>,
) -> Result<Json<serde_json::Value>> {
    debug!("Getting revenue stats for user: {}", user.id);

    let period = match query.period.as_deref() {
        Some("daily") => RevenuePeriod::Daily,
        Some("weekly") => RevenuePeriod::Weekly,
        Some("yearly") => RevenuePeriod::Yearly,
        Some("custom") => RevenuePeriod::Custom,
        _ => RevenuePeriod::Monthly,
    };

    // 如果没有指定日期范围，使用当月
    let now = chrono::Utc::now();
    let (start_date, end_date) = match (query.start_date, query.end_date) {
        (Some(start), Some(end)) => (start, end),
        _ => {
            let current_month_start = chrono::TimeZone::from_utc_datetime(
                &chrono::Utc,
                &chrono::NaiveDate::from_ymd_opt(now.year(), now.month(), 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
            );
            let next_month_start = if now.month() == 12 {
                chrono::TimeZone::from_utc_datetime(
                    &chrono::Utc,
                    &chrono::NaiveDate::from_ymd_opt(now.year() + 1, 1, 1)
                        .unwrap()
                        .and_hms_opt(0, 0, 0)
                        .unwrap()
                )
            } else {
                chrono::TimeZone::from_utc_datetime(
                    &chrono::Utc,
                    &chrono::NaiveDate::from_ymd_opt(now.year(), now.month() + 1, 1)
                        .unwrap()
                        .and_hms_opt(0, 0, 0)
                        .unwrap()
                )
            };
            (current_month_start, next_month_start)
        }
    };

    let stats = state.revenue_service
        .get_revenue_stats(&user.id, period, start_date, end_date)
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": stats
    })))
}

#[derive(Debug, Deserialize)]
struct TransactionsQuery {
    page: Option<i32>,
    per_page: Option<i32>,
    source_type: Option<String>,
    status: Option<String>,
}

/// 获取收益交易记录
async fn get_revenue_transactions(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TransactionsQuery>,
    Extension(user): Extension<User>,
) -> Result<Json<serde_json::Value>> {
    debug!("Getting revenue transactions for user: {}", user.id);

    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let (transactions, total) = state.revenue_service
        .query_revenue_transactions(
            &user.id,
            query.source_type.as_deref(),
            query.status.as_deref(),
            offset,
            per_page,
        )
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": {
            "transactions": transactions,
            "pagination": {
                "page": page,
                "per_page": per_page,
                "total": total,
                "pages": (total + per_page as i64 - 1) / per_page as i64
            }
        }
    })))
}

/// 创建支付
async fn create_payout(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Json(payload): Json<CreatePayoutRequest>,
) -> Result<Json<serde_json::Value>> {
    debug!("Creating payout for user: {}", user.id);

    let payout = state.revenue_service
        .create_payout(&user.id, payload)
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": payout
    })))
}

/// 获取支付列表
async fn get_payouts(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
) -> Result<Json<serde_json::Value>> {
    debug!("Getting payouts for user: {}", user.id);


    let payouts = state.revenue_service
        .query_payouts(&user.id)
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": payouts
    })))
}

/// 获取支付详情
async fn get_payout_details(
    State(state): State<Arc<AppState>>,
    Path(payout_id): Path<String>,
    Extension(user): Extension<User>,
) -> Result<Json<serde_json::Value>> {
    debug!("Getting payout details: {} for user: {}", payout_id, user.id);


    if let Some(payout) = state.revenue_service
        .query_payout_details(&payout_id, &user.id)
        .await? {
        Ok(Json(serde_json::json!({
            "success": true,
            "data": payout
        })))
    } else {
        Err(AppError::NotFound("支付记录不存在".to_string()))
    }
}

/// 获取银行账户列表
async fn get_bank_accounts(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
) -> Result<Json<serde_json::Value>> {
    debug!("Getting bank accounts for user: {}", user.id);

    let bank_accounts = state.revenue_service
        .get_bank_accounts(&user.id)
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": bank_accounts
    })))
}

#[derive(Debug, Deserialize)]
struct AddBankAccountRequest {
    account_holder_name: String,
    bank_name: String,
    country: String,
    currency: String,
    stripe_bank_account_token: String,
}

/// 添加银行账户
async fn add_bank_account(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Json(payload): Json<AddBankAccountRequest>,
) -> Result<Json<serde_json::Value>> {
    debug!("Adding bank account for user: {}", user.id);

    // TODO: 与Stripe集成验证银行账户
    let account = state.revenue_service
        .add_bank_account(
            &user.id,
            &payload.account_holder_name,
            &payload.bank_name,
            &payload.country,
            &payload.currency,
        )
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": account,
        "message": "银行账户已添加，等待验证"
    })))
}

/// 验证银行账户
async fn verify_bank_account(
    State(state): State<Arc<AppState>>,
    Path(account_id): Path<String>,
    Extension(user): Extension<User>,
) -> Result<Json<serde_json::Value>> {
    debug!("Verifying bank account: {} for user: {}", account_id, user.id);

    // TODO: 实现实际的银行账户验证逻辑
    let success = state.revenue_service
        .verify_bank_account(&account_id, &user.id)
        .await?;
    
    if !success {
        return Err(AppError::NotFound("银行账户不存在".to_string()));
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "银行账户验证成功"
    })))
}

/// 设置默认银行账户
async fn set_default_bank_account(
    State(state): State<Arc<AppState>>,
    Path(account_id): Path<String>,
    Extension(user): Extension<User>,
) -> Result<Json<serde_json::Value>> {
    debug!("Setting default bank account: {} for user: {}", account_id, user.id);

    let success = state.revenue_service
        .set_default_bank_account(&account_id, &user.id)
        .await?;
    
    if !success {
        return Err(AppError::BadRequest("银行账户不存在或未验证".to_string()));
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "默认银行账户设置成功"
    })))
}

/// 获取收益设置
async fn get_revenue_settings(
    State(_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
) -> Result<Json<serde_json::Value>> {
    debug!("Getting revenue settings for user: {}", user.id);

    // 返回收益分成配置和其他设置
    let settings = serde_json::json!({
        "revenue_share": RevenueShare::default(),
        "minimum_payout_amount": 5000, // $50
        "payout_schedule": "monthly",
        "payout_day": 1,
        "auto_payout_enabled": false,
        "tax_reporting_enabled": false
    });

    Ok(Json(serde_json::json!({
        "success": true,
        "data": settings
    })))
}

#[derive(Debug, Deserialize)]
struct UpdateRevenueSettingsRequest {
    auto_payout_enabled: Option<bool>,
    minimum_auto_payout_amount: Option<i64>,
    tax_reporting_enabled: Option<bool>,
}

/// 更新收益设置
async fn update_revenue_settings(
    State(_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Json(_payload): Json<UpdateRevenueSettingsRequest>,
) -> Result<Json<serde_json::Value>> {
    debug!("Updating revenue settings for user: {}", user.id);

    // TODO: 实现设置更新逻辑
    // 目前返回成功响应
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "收益设置更新成功"
    })))
}