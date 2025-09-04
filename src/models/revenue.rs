use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use validator::Validate;

/// 收益记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueRecord {
    pub id: String,
    pub creator_id: String,
    pub source_type: RevenueSourceType,
    pub source_id: String, // 订阅ID或购买ID
    pub amount: i64, // 收益金额（美分）
    pub currency: String,
    pub status: RevenueStatus,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
}

/// 收益来源类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RevenueSourceType {
    Subscription,      // 订阅收益
    ArticlePurchase,   // 文章单次购买
    Tip,              // 打赏
    Advertisement,     // 广告收益
}

/// 收益状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RevenueStatus {
    Pending,    // 待处理
    Processing, // 处理中
    Completed,  // 已完成
    Failed,     // 失败
    Cancelled,  // 已取消
}

/// 作者收益汇总
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatorEarnings {
    pub creator_id: String,
    pub total_earnings: i64, // 总收益（美分）
    pub available_balance: i64, // 可提现余额
    pub pending_balance: i64, // 待结算余额
    pub lifetime_earnings: i64, // 历史总收益
    pub currency: String,
    pub last_payout_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

/// 收益支付记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payout {
    pub id: String,
    pub creator_id: String,
    pub amount: i64, // 支付金额（美分）
    pub currency: String,
    pub method: PayoutMethod,
    pub status: PayoutStatus,
    pub stripe_payout_id: Option<String>,
    pub bank_account_id: Option<String>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
    pub failed_at: Option<DateTime<Utc>>,
    pub failure_reason: Option<String>,
}

/// 支付方式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PayoutMethod {
    Stripe,        // Stripe Connect
    BankTransfer,  // 银行转账
    Paypal,        // PayPal
    Manual,        // 手动处理
}

/// 支付状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PayoutStatus {
    Pending,    // 待处理
    Processing, // 处理中
    Completed,  // 已完成
    Failed,     // 失败
    Cancelled,  // 已取消
}

/// 收益统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueStats {
    pub period: RevenuePeriod,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub subscription_revenue: i64,
    pub purchase_revenue: i64,
    pub tip_revenue: i64,
    pub ad_revenue: i64,
    pub total_revenue: i64,
    pub transaction_count: i32,
    pub new_subscribers: i32,
    pub cancelled_subscribers: i32,
    pub top_earning_content: Vec<ContentEarning>,
}

/// 收益统计周期
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RevenuePeriod {
    Daily,
    Weekly,
    Monthly,
    Yearly,
    Custom,
}

/// 内容收益
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentEarning {
    pub content_id: String,
    pub content_type: String, // article, publication, series
    pub title: String,
    pub subscription_revenue: i64,
    pub purchase_revenue: i64,
    pub total_revenue: i64,
    pub view_count: i64,
    pub conversion_rate: f64,
}

/// 银行账户信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankAccount {
    pub id: String,
    pub creator_id: String,
    pub account_holder_name: String,
    pub account_number_last4: String, // 只存储后4位
    pub bank_name: String,
    pub country: String,
    pub currency: String,
    pub is_default: bool,
    pub is_verified: bool,
    pub stripe_bank_account_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub verified_at: Option<DateTime<Utc>>,
}

/// 创建支付请求
#[derive(Debug, Validate, Deserialize)]
pub struct CreatePayoutRequest {
    #[validate(range(min = 100))] // 最低1美元
    pub amount: i64,
    
    #[validate(length(max = 500))]
    pub description: Option<String>,
    
    pub bank_account_id: Option<String>,
}

/// 收益查询参数
#[derive(Debug, Deserialize)]
pub struct RevenueQuery {
    pub creator_id: Option<String>,
    pub source_type: Option<RevenueSourceType>,
    pub status: Option<RevenueStatus>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub page: Option<i32>,
    pub per_page: Option<i32>,
}

/// 收益仪表板
#[derive(Debug, Serialize)]
pub struct RevenueDashboard {
    pub earnings: CreatorEarnings,
    pub current_month_stats: RevenueStats,
    pub last_month_stats: RevenueStats,
    pub recent_transactions: Vec<RevenueRecord>,
    pub pending_payouts: Vec<Payout>,
    pub bank_accounts: Vec<BankAccount>,
    pub minimum_payout_amount: i64,
    pub next_payout_date: Option<DateTime<Utc>>,
}

/// 收益分成配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueShare {
    pub platform_fee_percentage: f64, // 平台费用百分比
    pub payment_processing_fee: f64,   // 支付处理费用百分比
    pub creator_share_percentage: f64, // 创作者分成百分比
}

impl Default for RevenueShare {
    fn default() -> Self {
        Self {
            platform_fee_percentage: 10.0,     // 平台收取10%
            payment_processing_fee: 2.9,       // 支付处理费2.9%
            creator_share_percentage: 87.1,    // 创作者获得87.1%
        }
    }
}

/// 计算创作者实际收益
pub fn calculate_creator_revenue(gross_amount: i64, revenue_share: &RevenueShare) -> i64 {
    let creator_share = gross_amount as f64 * (revenue_share.creator_share_percentage / 100.0);
    creator_share.round() as i64
}

/// 计算平台费用
pub fn calculate_platform_fee(gross_amount: i64, revenue_share: &RevenueShare) -> i64 {
    let platform_fee = gross_amount as f64 * (revenue_share.platform_fee_percentage / 100.0);
    platform_fee.round() as i64
}

/// 计算支付处理费
pub fn calculate_processing_fee(gross_amount: i64, revenue_share: &RevenueShare) -> i64 {
    let processing_fee = gross_amount as f64 * (revenue_share.payment_processing_fee / 100.0);
    processing_fee.round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_revenue_calculation() {
        let revenue_share = RevenueShare::default();
        let gross_amount = 10000; // $100.00
        
        let creator_revenue = calculate_creator_revenue(gross_amount, &revenue_share);
        let platform_fee = calculate_platform_fee(gross_amount, &revenue_share);
        let processing_fee = calculate_processing_fee(gross_amount, &revenue_share);
        
        assert_eq!(creator_revenue, 8710); // $87.10
        assert_eq!(platform_fee, 1000);    // $10.00
        assert_eq!(processing_fee, 290);    // $2.90
        assert_eq!(creator_revenue + platform_fee + processing_fee, gross_amount);
    }
}