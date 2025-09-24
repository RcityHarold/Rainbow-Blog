use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Stripe客户配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripeCustomer {
    pub id: String,
    pub user_id: String,
    pub stripe_customer_id: String,
    pub email: String,
    pub name: Option<String>,
    pub default_payment_method: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Stripe支付方式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripePaymentMethod {
    pub id: String,
    pub user_id: String,
    pub stripe_payment_method_id: String,
    pub payment_method_type: PaymentMethodType,
    pub card_brand: Option<String>,
    pub card_last4: Option<String>,
    pub card_exp_month: Option<i32>,
    pub card_exp_year: Option<i32>,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 支付方式类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMethodType {
    Card,
    BankAccount,
    Alipay,
    Wechat,
}

/// Stripe意图模式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StripeIntentMode {
    Payment,
    Setup,
}

impl Default for StripeIntentMode {
    fn default() -> Self {
        StripeIntentMode::Payment
    }
}

/// Stripe订阅
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripeSubscription {
    pub id: String,
    pub subscription_id: String, // 内部订阅ID
    pub stripe_subscription_id: String,
    pub stripe_customer_id: String,
    pub stripe_price_id: String,
    pub status: StripeSubscriptionStatus,
    pub current_period_start: DateTime<Utc>,
    pub current_period_end: DateTime<Utc>,
    pub cancel_at_period_end: bool,
    pub canceled_at: Option<DateTime<Utc>>,
    pub trial_start: Option<DateTime<Utc>>,
    pub trial_end: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Stripe订阅状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StripeSubscriptionStatus {
    Trialing,
    Active,
    PastDue,
    Canceled,
    Unpaid,
    Incomplete,
    IncompleteExpired,
}

/// Stripe支付意图
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripePaymentIntent {
    pub id: String,
    pub stripe_payment_intent_id: String,
    pub user_id: String,
    pub amount: i64,
    pub currency: String,
    pub status: PaymentIntentStatus,
    pub mode: StripeIntentMode,
    pub payment_method_id: Option<String>,
    pub article_id: Option<String>, // 文章购买
    pub metadata: serde_json::Value,
    pub client_secret: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripeIntentResponse {
    pub mode: StripeIntentMode,
    pub client_secret: String,
    pub payment_intent: Option<StripePaymentIntent>,
    pub setup_intent_id: Option<String>,
}

/// 支付意图状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentIntentStatus {
    RequiresPaymentMethod,
    RequiresConfirmation,
    RequiresAction,
    Processing,
    RequiresCapture,
    Canceled,
    Succeeded,
}

/// Stripe Connect账户
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripeConnectAccount {
    pub id: String,
    pub user_id: String,
    pub stripe_account_id: String,
    pub account_type: ConnectAccountType,
    pub country: String,
    pub currency: String,
    pub details_submitted: bool,
    pub charges_enabled: bool,
    pub payouts_enabled: bool,
    pub requirements: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectAccountResponse {
    pub account: StripeConnectAccount,
    pub onboarding_url: Option<String>,
    pub requires_onboarding: bool,
}

/// Connect账户类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectAccountType {
    Express,
    Standard,
    Custom,
}

/// Stripe产品
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripeProduct {
    pub id: String,
    pub plan_id: String, // 内部订阅计划ID
    pub stripe_product_id: String,
    pub name: String,
    pub description: Option<String>,
    pub active: bool,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Stripe价格
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripePrice {
    pub id: String,
    pub product_id: String, // StripeProduct的ID
    pub stripe_price_id: String,
    pub currency: String,
    pub unit_amount: i64,
    pub recurring_interval: Option<String>, // month, year
    pub recurring_interval_count: Option<i32>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// WebHook事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripeWebhookEvent {
    pub id: String,
    pub stripe_event_id: String,
    pub event_type: String,
    pub processed: bool,
    pub processed_at: Option<DateTime<Utc>>,
    pub data: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// 创建支付意图请求
#[derive(Debug, Deserialize)]
pub struct CreateStripeIntentRequest {
    #[serde(default)]
    pub mode: StripeIntentMode,

    #[serde(default)]
    pub amount: Option<i64>,

    #[serde(default)]
    pub currency: Option<String>,

    pub payment_method_id: Option<String>,
    pub article_id: Option<String>,
    pub confirm: Option<bool>,
    pub metadata: Option<serde_json::Value>,
}

/// 添加支付方式请求
#[derive(Debug, Deserialize)]
pub struct CreatePaymentMethodRequest {
    pub payment_method_id: String,
    #[serde(default)]
    pub set_as_default: bool,
}

/// 创建订阅请求
#[derive(Debug, Validate, Deserialize)]
pub struct CreateStripeSubscriptionRequest {
    pub price_id: String,
    pub payment_method_id: Option<String>,
    pub trial_period_days: Option<i32>,
    pub coupon: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// 创建Connect账户请求
#[derive(Debug, Validate, Deserialize)]
pub struct CreateConnectAccountRequest {
    #[validate(length(min = 2, max = 2))]
    pub country: String,

    pub account_type: ConnectAccountType,
    pub email: String,
    pub business_type: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// 支付配置
#[derive(Debug, Clone)]
pub struct StripeConfig {
    pub secret_key: String,
    pub publishable_key: String,
    pub webhook_endpoint_secret: String,
    pub connect_client_id: Option<String>,
    pub connect_return_url: Option<String>,
    pub connect_refresh_url: Option<String>,
    pub api_version: String,
}

impl Default for StripeConfig {
    fn default() -> Self {
        let frontend_url =
            std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:3001".to_string());
        let frontend_base = frontend_url.trim_end_matches('/');
        let default_return = std::env::var("STRIPE_CONNECT_RETURN_URL").ok().or_else(|| {
            Some(format!(
                "{}/settings#billing?connect=success",
                frontend_base
            ))
        });
        let default_refresh = std::env::var("STRIPE_CONNECT_REFRESH_URL")
            .ok()
            .or_else(|| {
                Some(format!(
                    "{}/settings#billing?connect=refresh",
                    frontend_base
                ))
            });

        Self {
            secret_key: std::env::var("STRIPE_SECRET_KEY").unwrap_or_default(),
            publishable_key: std::env::var("STRIPE_PUBLISHABLE_KEY").unwrap_or_default(),
            webhook_endpoint_secret: std::env::var("STRIPE_WEBHOOK_SECRET").unwrap_or_default(),
            connect_client_id: std::env::var("STRIPE_CONNECT_CLIENT_ID").ok(),
            connect_return_url: default_return,
            connect_refresh_url: default_refresh,
            api_version: "2023-10-16".to_string(),
        }
    }
}

/// Stripe错误类型
#[derive(Debug, Serialize, Deserialize)]
pub struct StripeError {
    pub code: Option<String>,
    pub message: String,
    pub param: Option<String>,
    pub error_type: String,
}

/// 支付统计
#[derive(Debug, Serialize)]
pub struct PaymentStats {
    pub total_payments: i64,
    pub successful_payments: i64,
    pub failed_payments: i64,
    pub total_amount: i64,
    pub average_amount: f64,
    pub currency: String,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
}

/// 退款记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripeRefund {
    pub id: String,
    pub stripe_refund_id: String,
    pub payment_intent_id: String,
    pub amount: i64,
    pub currency: String,
    pub reason: Option<String>,
    pub status: RefundStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 退款状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RefundStatus {
    Pending,
    Succeeded,
    Failed,
    Canceled,
}

/// 优惠券
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripeCoupon {
    pub id: String,
    pub stripe_coupon_id: String,
    pub name: String,
    pub percent_off: Option<i32>,
    pub amount_off: Option<i64>,
    pub currency: Option<String>,
    pub duration: CouponDuration,
    pub duration_in_months: Option<i32>,
    pub max_redemptions: Option<i32>,
    pub times_redeemed: i32,
    pub valid: bool,
    pub redeem_by: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 优惠券持续时间
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CouponDuration {
    Once,
    Repeating,
    Forever,
}

/// 发票
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripeInvoice {
    pub id: String,
    pub stripe_invoice_id: String,
    pub subscription_id: Option<String>,
    pub customer_id: String,
    pub amount_paid: i64,
    pub amount_due: i64,
    pub currency: String,
    pub status: InvoiceStatus,
    pub hosted_invoice_url: Option<String>,
    pub invoice_pdf: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 发票状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InvoiceStatus {
    Draft,
    Open,
    Paid,
    Uncollectible,
    Void,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stripe_config_default() {
        let config = StripeConfig::default();
        assert_eq!(config.api_version, "2023-10-16");
    }

    #[test]
    fn test_payment_method_type_serialization() {
        let payment_type = PaymentMethodType::Card;
        let serialized = serde_json::to_string(&payment_type).unwrap();
        assert_eq!(serialized, "\"card\"");
    }

    #[test]
    fn test_subscription_status_serialization() {
        let status = StripeSubscriptionStatus::Active;
        let serialized = serde_json::to_string(&status).unwrap();
        assert_eq!(serialized, "\"active\"");
    }
}
