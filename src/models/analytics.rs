use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// 用户统计概览
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAnalyticsOverview {
    pub total_articles: i64,
    pub total_views: i64,
    pub total_claps: i64,
    pub total_comments: i64,
    pub total_bookmarks: i64,
    pub total_shares: i64,
    pub total_followers: i64,
    pub total_following: i64,
    pub avg_reading_time: f64,
    pub avg_claps_per_article: f64,
    pub avg_views_per_article: f64,
    pub engagement_rate: f64,
}

/// 文章统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleAnalytics {
    pub article_id: String,
    pub title: String,
    pub slug: String,
    pub views: i64,
    pub unique_viewers: i64,
    pub claps: i64,
    pub comments: i64,
    pub bookmarks: i64,
    pub shares: i64,
    pub avg_read_time: f64,
    pub bounce_rate: f64,
    pub engagement_rate: f64,
    pub published_at: DateTime<Utc>,
}

/// 时间段统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRangeAnalytics {
    pub date: DateTime<Utc>,
    pub views: i64,
    pub claps: i64,
    pub comments: i64,
    pub bookmarks: i64,
    pub new_followers: i64,
    pub articles_published: i64,
}

/// 受众分析
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudienceAnalytics {
    pub total_readers: i64,
    pub returning_readers: i64,
    pub new_readers: i64,
    pub avg_session_duration: f64,
    pub top_referrers: Vec<ReferrerInfo>,
    pub device_breakdown: DeviceBreakdown,
    pub geographic_distribution: Vec<GeographicInfo>,
}

/// 推荐来源信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferrerInfo {
    pub source: String,
    pub count: i64,
    pub percentage: f64,
}

/// 设备分布
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceBreakdown {
    pub desktop: i64,
    pub mobile: i64,
    pub tablet: i64,
}

/// 地理分布
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeographicInfo {
    pub country: String,
    pub count: i64,
    pub percentage: f64,
}

/// 标签分析
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagAnalytics {
    pub tag_id: String,
    pub name: String,
    pub total_articles: i64,
    pub total_views: i64,
    pub total_claps: i64,
    pub avg_engagement: f64,
    pub growth_rate: f64,
}

/// 收入分析（如果有付费内容）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueAnalytics {
    pub total_revenue: f64,
    pub paid_subscribers: i64,
    pub conversion_rate: f64,
    pub avg_revenue_per_user: f64,
    pub monthly_recurring_revenue: f64,
    pub churn_rate: f64,
}

/// 趋势分析
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAnalytics {
    pub period: String, // "daily", "weekly", "monthly"
    pub metrics: Vec<TrendDataPoint>,
    pub growth_percentage: f64,
    pub peak_day: DateTime<Utc>,
    pub peak_value: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendDataPoint {
    pub date: DateTime<Utc>,
    pub value: i64,
    pub label: String,
}

/// 内容表现分析
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentPerformance {
    pub best_performing_articles: Vec<ArticleAnalytics>,
    pub underperforming_articles: Vec<ArticleAnalytics>,
    pub optimal_publish_times: Vec<OptimalTimeSlot>,
    pub content_suggestions: Vec<ContentSuggestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimalTimeSlot {
    pub day_of_week: String,
    pub hour: i32,
    pub avg_engagement: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentSuggestion {
    pub suggestion_type: SuggestionType,
    pub message: String,
    pub priority: Priority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuggestionType {
    Topic,
    Length,
    PublishTime,
    TagUsage,
    SeriesCreation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Priority {
    High,
    Medium,
    Low,
}

/// 统计查询参数
#[derive(Debug, Clone, Deserialize)]
pub struct AnalyticsQuery {
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub period: Option<AnalyticsPeriod>,
    pub metric: Option<MetricType>,
    pub limit: Option<i32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnalyticsPeriod {
    Hour,
    Day,
    Week,
    Month,
    Quarter,
    Year,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricType {
    Views,
    Claps,
    Comments,
    Bookmarks,
    Followers,
    Revenue,
    Engagement,
    All,
}

/// 实时统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeAnalytics {
    pub active_readers: i64,
    pub articles_being_read: Vec<ArticleReadInfo>,
    pub recent_interactions: Vec<InteractionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleReadInfo {
    pub article_id: String,
    pub title: String,
    pub reader_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionInfo {
    pub interaction_type: String,
    pub article_id: String,
    pub article_title: String,
    pub user_name: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// 综合仪表板
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsDashboard {
    pub overview: UserAnalyticsOverview,
    pub recent_articles: Vec<ArticleAnalytics>,
    pub time_series: Vec<TimeRangeAnalytics>,
    pub audience: AudienceAnalytics,
    pub top_tags: Vec<TagAnalytics>,
    pub content_performance: ContentPerformance,
    pub trends: TrendAnalytics,
    pub revenue: Option<RevenueAnalytics>,
    pub realtime: RealtimeAnalytics,
}

/// 导出选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportOptions {
    pub format: ExportFormat,
    pub include_raw_data: bool,
    pub date_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Csv,
    Json,
    Excel,
    Pdf,
}