use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use validator::Validate;

/// 付费内容访问控制
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentAccess {
    pub article_id: String,
    pub user_id: String,
    pub has_access: bool,
    pub access_type: AccessType,
    pub subscription_id: Option<String>,
    pub granted_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// 访问类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AccessType {
    Free,           // 免费内容
    Subscription,   // 订阅访问
    OneTime,        // 单次购买
    Author,         // 作者本人
    Preview,        // 预览访问（部分内容）
}

/// 付费内容预览
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentPreview {
    pub article_id: String,
    pub preview_content: String,
    pub preview_html: String,
    pub is_complete: bool,
    pub paywall_message: String,
    pub subscription_required: bool,
    pub creator_id: String,
}

/// 内容访问请求
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ContentAccessRequest {
    pub article_id: String,
}

/// 文章收费设置
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ArticlePricingRequest {
    #[validate(range(min = 0, message = "价格不能为负数"))]
    pub price: Option<i64>, // 单次购买价格（美分），None表示仅订阅
    
    pub subscription_required: bool, // 是否需要订阅
    
    #[validate(range(min = 0, max = 100, message = "预览比例必须在0-100之间"))]
    pub preview_percentage: Option<u8>, // 预览内容比例（0-100）
    
    #[validate(length(max = 200, message = "付费墙信息不能超过200字符"))]
    pub paywall_message: Option<String>, // 自定义付费墙信息
}

/// 文章定价信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticlePricing {
    pub article_id: String,
    pub is_paid_content: bool,
    pub price: Option<i64>, // 单次购买价格
    pub subscription_required: bool,
    pub preview_percentage: u8,
    pub paywall_message: String,
    pub creator_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 单次购买记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticlePurchase {
    pub id: String,
    pub article_id: String,
    pub buyer_id: String,
    pub creator_id: String,
    pub amount: i64, // 支付金额（美分）
    pub currency: String,
    pub stripe_payment_intent_id: Option<String>,
    pub status: PurchaseStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 购买状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PurchaseStatus {
    Pending,    // 待支付
    Completed,  // 已完成
    Failed,     // 支付失败
    Refunded,   // 已退款
}

/// 单次购买请求
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ArticlePurchaseRequest {
    pub article_id: String,
    pub payment_method_id: Option<String>, // Stripe payment method ID
}

/// 内容访问统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentAccessStats {
    pub article_id: String,
    pub total_views: i64,
    pub free_views: i64,
    pub subscription_views: i64,
    pub purchase_views: i64,
    pub preview_views: i64,
    pub conversion_rate: f64, // 从预览到付费的转换率
    pub total_revenue: i64,   // 总收入（美分）
}

/// 用户内容访问历史
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserContentAccess {
    pub user_id: String,
    pub article_id: String,
    pub access_type: AccessType,
    pub accessed_at: DateTime<Utc>,
    pub reading_time: Option<i64>, // 阅读时间（秒）
    pub completed: bool, // 是否完整阅读
}

/// 付费内容仪表板
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentDashboard {
    pub creator_id: String,
    pub total_paid_articles: i64,
    pub total_subscribers: i64,
    pub total_purchases: i64,
    pub monthly_revenue: i64,
    pub top_earning_articles: Vec<ArticleEarnings>,
    pub recent_purchases: Vec<ArticlePurchase>,
    pub access_stats: Vec<ContentAccessStats>,
}

/// 文章收益信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleEarnings {
    pub article_id: String,
    pub title: String,
    pub slug: String,
    pub total_revenue: i64,
    pub subscription_revenue: i64,
    pub purchase_revenue: i64,
    pub view_count: i64,
    pub purchase_count: i64,
}

/// 收益分析查询参数
#[derive(Debug, Clone, Deserialize)]
pub struct EarningsQuery {
    pub creator_id: Option<String>,
    pub article_id: Option<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub limit: Option<i32>,
}

/// 付费内容设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentSettings {
    pub creator_id: String,
    pub default_preview_percentage: u8,
    pub default_paywall_message: String,
    pub auto_paywall_enabled: bool, // 新文章自动设为付费
    pub subscription_required_by_default: bool,
    pub allow_single_purchases: bool,
    pub min_article_price: i64, // 最低文章价格
}