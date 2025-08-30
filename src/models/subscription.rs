use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use validator::Validate;

/// 订阅计划
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionPlan {
    pub id: String,
    pub creator_id: String,
    pub name: String,
    pub description: Option<String>,
    pub price: i64, // 价格（美分）
    pub currency: String,
    pub benefits: Vec<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 创建订阅计划请求
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateSubscriptionPlanRequest {
    #[validate(length(min = 1, max = 100, message = "计划名称长度必须在1-100字符之间"))]
    pub name: String,
    
    #[validate(length(max = 500, message = "描述不能超过500字符"))]
    pub description: Option<String>,
    
    #[validate(range(min = 0, message = "价格不能为负数"))]
    pub price: i64, // 月费（美分）
    
    #[validate(length(min = 3, max = 3, message = "货币代码必须是3位字符"))]
    pub currency: Option<String>, // 默认USD
    
    pub benefits: Vec<String>,
}

/// 更新订阅计划请求
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UpdateSubscriptionPlanRequest {
    #[validate(length(min = 1, max = 100, message = "计划名称长度必须在1-100字符之间"))]
    pub name: Option<String>,
    
    #[validate(length(max = 500, message = "描述不能超过500字符"))]
    pub description: Option<String>,
    
    #[validate(range(min = 0, message = "价格不能为负数"))]
    pub price: Option<i64>,
    
    pub benefits: Option<Vec<String>>,
    pub is_active: Option<bool>,
}

/// 用户订阅
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: String,
    pub subscriber_id: String,
    pub plan_id: String,
    pub creator_id: String,
    pub status: SubscriptionStatus,
    pub started_at: DateTime<Utc>,
    pub current_period_end: DateTime<Utc>,
    pub canceled_at: Option<DateTime<Utc>>,
    pub stripe_subscription_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 订阅状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    Active,
    Canceled,
    Expired,
    PastDue,
}

impl std::fmt::Display for SubscriptionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Canceled => write!(f, "canceled"),
            Self::Expired => write!(f, "expired"),
            Self::PastDue => write!(f, "past_due"),
        }
    }
}

/// 创建订阅请求
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateSubscriptionRequest {
    pub plan_id: String,
    pub payment_method_id: Option<String>, // Stripe payment method ID
}

/// 订阅详情（包含计划信息）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionDetails {
    pub id: String,
    pub subscriber_id: String,
    pub plan: SubscriptionPlan,
    pub creator: SubscriptionCreator,
    pub status: SubscriptionStatus,
    pub started_at: DateTime<Utc>,
    pub current_period_end: DateTime<Utc>,
    pub canceled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 订阅中的创作者信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionCreator {
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub is_verified: bool,
}

/// 订阅统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionStats {
    pub total_subscribers: i64,
    pub active_subscribers: i64,
    pub monthly_revenue: i64, // 美分
    pub total_revenue: i64, // 美分
    pub churn_rate: f64, // 流失率
    pub growth_rate: f64, // 增长率
}

/// 订阅查询参数
#[derive(Debug, Clone, Deserialize)]
pub struct SubscriptionQuery {
    pub page: Option<i32>,
    pub limit: Option<i32>,
    pub status: Option<SubscriptionStatus>,
    pub creator_id: Option<String>,
    pub subscriber_id: Option<String>,
}

/// 订阅分页结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionListResponse {
    pub subscriptions: Vec<SubscriptionDetails>,
    pub total: i64,
    pub page: i32,
    pub limit: i32,
    pub total_pages: i32,
}

/// Stripe Webhook事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripeWebhookEvent {
    pub id: String,
    pub r#type: String,
    pub data: serde_json::Value,
}

/// 订阅计划分页结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionPlanListResponse {
    pub plans: Vec<SubscriptionPlan>,
    pub total: i64,
    pub page: i32,
    pub limit: i32,
    pub total_pages: i32,
}

/// 订阅计划查询参数
#[derive(Debug, Clone, Deserialize)]
pub struct SubscriptionPlanQuery {
    pub page: Option<i32>,
    pub limit: Option<i32>,
    pub creator_id: Option<String>,
    pub is_active: Option<bool>,
}

/// 创作者收益统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatorRevenue {
    pub creator_id: String,
    pub total_subscribers: i64,
    pub monthly_revenue: i64,
    pub total_revenue: i64,
    pub subscription_plans: Vec<SubscriptionPlan>,
    pub recent_subscriptions: Vec<SubscriptionDetails>,
}

/// 订阅检查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionCheck {
    pub is_subscribed: bool,
    pub subscription: Option<SubscriptionDetails>,
    pub can_access_paid_content: bool,
}