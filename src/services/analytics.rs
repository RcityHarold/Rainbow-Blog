use crate::{
    error::{AppError, Result},
    models::analytics::*,
    services::Database,
};
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};
use std::sync::Arc;
use std::collections::HashMap;
use tracing::{debug, info};

#[derive(Clone)]
pub struct AnalyticsService {
    db: Arc<Database>,
}

impl AnalyticsService {
    pub async fn new(db: Arc<Database>) -> Result<Self> {
        Ok(Self { db })
    }

    /// 获取用户的综合统计仪表板
    pub async fn get_user_dashboard(
        &self,
        user_id: &str,
        query: AnalyticsQuery,
    ) -> Result<AnalyticsDashboard> {
        debug!("Getting analytics dashboard for user: {}", user_id);

        let end_date = query.end_date.unwrap_or_else(Utc::now);
        let start_date = query.start_date.unwrap_or(end_date - Duration::days(30));

        let overview = self.get_user_overview(user_id).await?;
        let recent_articles = self.get_recent_article_analytics(user_id, 10).await?;
        let time_series = self.get_time_series_data(user_id, &start_date, &end_date, query.period).await?;
        let audience = self.get_audience_analytics(user_id, &start_date, &end_date).await?;
        let top_tags = self.get_top_tags_analytics(user_id, 10).await?;
        let content_performance = self.get_content_performance(user_id).await?;
        let trends = self.get_trend_analytics(user_id, &start_date, &end_date).await?;
        let revenue = self.get_revenue_analytics(user_id).await.ok();
        let realtime = self.get_realtime_analytics(user_id).await?;

        Ok(AnalyticsDashboard {
            overview,
            recent_articles,
            time_series,
            audience,
            top_tags,
            content_performance,
            trends,
            revenue,
            realtime,
        })
    }

    /// 获取用户统计概览
    pub async fn get_user_overview(&self, user_id: &str) -> Result<UserAnalyticsOverview> {
        debug!("Getting user overview for: {}", user_id);

        let query = r#"
            SELECT 
                COUNT(DISTINCT a.id) as total_articles,
                SUM(a.view_count) as total_views,
                SUM(a.clap_count) as total_claps,
                SUM(a.comment_count) as total_comments,
                SUM(a.bookmark_count) as total_bookmarks,
                SUM(a.share_count) as total_shares,
                AVG(a.reading_time) as avg_reading_time,
                AVG(a.clap_count) as avg_claps_per_article,
                AVG(a.view_count) as avg_views_per_article
            FROM article a
            WHERE a.author_id = $user_id
            AND a.status = 'published'
            AND a.is_deleted = false
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "user_id": user_id
        })).await?;

        let stats: Vec<Value> = response.take(0)?;
        let stat = stats.first().ok_or_else(|| AppError::NotFound("Stats not found".to_string()))?;

        // 获取关注者数量
        let follower_count = self.get_follower_count(user_id).await?;
        let following_count = self.get_following_count(user_id).await?;

        let total_views = stat["total_views"].as_i64().unwrap_or(0);
        let total_claps = stat["total_claps"].as_i64().unwrap_or(0);
        let total_comments = stat["total_comments"].as_i64().unwrap_or(0);
        let total_bookmarks = stat["total_bookmarks"].as_i64().unwrap_or(0);

        // 计算参与率
        let engagement_rate = if total_views > 0 {
            ((total_claps + total_comments + total_bookmarks) as f64 / total_views as f64) * 100.0
        } else {
            0.0
        };

        Ok(UserAnalyticsOverview {
            total_articles: stat["total_articles"].as_i64().unwrap_or(0),
            total_views,
            total_claps,
            total_comments,
            total_bookmarks,
            total_shares: stat["total_shares"].as_i64().unwrap_or(0),
            total_followers: follower_count,
            total_following: following_count,
            avg_reading_time: stat["avg_reading_time"].as_f64().unwrap_or(0.0),
            avg_claps_per_article: stat["avg_claps_per_article"].as_f64().unwrap_or(0.0),
            avg_views_per_article: stat["avg_views_per_article"].as_f64().unwrap_or(0.0),
            engagement_rate,
        })
    }

    /// 获取最近文章的分析数据
    pub async fn get_recent_article_analytics(
        &self,
        user_id: &str,
        limit: i32,
    ) -> Result<Vec<ArticleAnalytics>> {
        let query = r#"
            SELECT 
                a.id as article_id,
                a.title,
                a.slug,
                a.view_count as views,
                a.clap_count as claps,
                a.comment_count as comments,
                a.bookmark_count as bookmarks,
                a.share_count as shares,
                a.reading_time as avg_read_time,
                a.published_at,
                (a.clap_count + a.comment_count + a.bookmark_count) * 100.0 / NULLIF(a.view_count, 0) as engagement_rate
            FROM article a
            WHERE a.author_id = $user_id
            AND a.status = 'published'
            AND a.is_deleted = false
            ORDER BY a.published_at DESC
            LIMIT $limit
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "user_id": user_id,
            "limit": limit
        })).await?;

        let articles: Vec<Value> = response.take(0)?;
        let mut results = Vec::new();

        for article_data in articles {
            let article_id = article_data["article_id"].as_str().unwrap_or("");
            
            // 获取唯一查看者数量
            let unique_viewers = self.get_unique_viewers_count(article_id).await?;
            
            results.push(ArticleAnalytics {
                article_id: article_id.to_string(),
                title: article_data["title"].as_str().unwrap_or("").to_string(),
                slug: article_data["slug"].as_str().unwrap_or("").to_string(),
                views: article_data["views"].as_i64().unwrap_or(0),
                unique_viewers,
                claps: article_data["claps"].as_i64().unwrap_or(0),
                comments: article_data["comments"].as_i64().unwrap_or(0),
                bookmarks: article_data["bookmarks"].as_i64().unwrap_or(0),
                shares: article_data["shares"].as_i64().unwrap_or(0),
                avg_read_time: article_data["avg_read_time"].as_f64().unwrap_or(0.0),
                bounce_rate: 0.0, // TODO: 实现跳出率计算
                engagement_rate: article_data["engagement_rate"].as_f64().unwrap_or(0.0),
                published_at: article_data["published_at"]
                    .as_str()
                    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(Utc::now),
            });
        }

        Ok(results)
    }

    /// 获取时间序列数据
    pub async fn get_time_series_data(
        &self,
        user_id: &str,
        start_date: &DateTime<Utc>,
        end_date: &DateTime<Utc>,
        period: Option<AnalyticsPeriod>,
    ) -> Result<Vec<TimeRangeAnalytics>> {
        let period = period.unwrap_or(AnalyticsPeriod::Day);
        
        // 根据时间段聚合数据
        let date_format = match period {
            AnalyticsPeriod::Hour => "%Y-%m-%d %H:00",
            AnalyticsPeriod::Day => "%Y-%m-%d",
            AnalyticsPeriod::Week => "%Y-W%V",
            AnalyticsPeriod::Month => "%Y-%m",
            AnalyticsPeriod::Quarter => "%Y-Q",
            AnalyticsPeriod::Year => "%Y",
        };

        // 这里简化了查询，实际应该使用数据库的日期函数
        let query = r#"
            SELECT 
                DATE(a.published_at) as date,
                COUNT(DISTINCT CASE WHEN type = 'view' THEN id END) as views,
                COUNT(DISTINCT CASE WHEN type = 'clap' THEN id END) as claps,
                COUNT(DISTINCT CASE WHEN type = 'comment' THEN id END) as comments,
                COUNT(DISTINCT CASE WHEN type = 'bookmark' THEN id END) as bookmarks,
                COUNT(DISTINCT CASE WHEN a.id IS NOT NULL THEN a.id END) as articles_published
            FROM (
                SELECT 'view' as type, id, created_at FROM article_view WHERE author_id = $user_id
                UNION ALL
                SELECT 'clap' as type, id, created_at FROM clap WHERE article_id IN (SELECT id FROM article WHERE author_id = $user_id)
                UNION ALL
                SELECT 'comment' as type, id, created_at FROM comment WHERE article_id IN (SELECT id FROM article WHERE author_id = $user_id)
                UNION ALL
                SELECT 'bookmark' as type, id, created_at FROM bookmark WHERE article_id IN (SELECT id FROM article WHERE author_id = $user_id)
            ) events
            LEFT JOIN article a ON events.type = 'article' AND events.id = a.id
            WHERE events.created_at >= $start_date
            AND events.created_at <= $end_date
            GROUP BY date
            ORDER BY date
        "#;

        // 简化版本，获取每日的文章统计
        let simple_query = r#"
            SELECT 
                DATE(published_at) as date,
                SUM(view_count) as views,
                SUM(clap_count) as claps,
                SUM(comment_count) as comments,
                SUM(bookmark_count) as bookmarks,
                COUNT(*) as articles_published
            FROM article
            WHERE author_id = $user_id
            AND status = 'published'
            AND published_at >= $start_date
            AND published_at <= $end_date
            GROUP BY DATE(published_at)
            ORDER BY date
        "#;

        let mut response = self.db.query_with_params(simple_query, json!({
            "user_id": user_id,
            "start_date": start_date,
            "end_date": end_date
        })).await?;

        let data: Vec<Value> = response.take(0)?;
        let mut results = Vec::new();

        for item in data {
            results.push(TimeRangeAnalytics {
                date: item["date"]
                    .as_str()
                    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(Utc::now),
                views: item["views"].as_i64().unwrap_or(0),
                claps: item["claps"].as_i64().unwrap_or(0),
                comments: item["comments"].as_i64().unwrap_or(0),
                bookmarks: item["bookmarks"].as_i64().unwrap_or(0),
                new_followers: 0, // TODO: 实现新关注者统计
                articles_published: item["articles_published"].as_i64().unwrap_or(0),
            });
        }

        Ok(results)
    }

    /// 获取受众分析
    pub async fn get_audience_analytics(
        &self,
        user_id: &str,
        start_date: &DateTime<Utc>,
        end_date: &DateTime<Utc>,
    ) -> Result<AudienceAnalytics> {
        // 这是一个简化版本，实际应该有更复杂的用户追踪
        let total_readers = self.get_total_readers_count(user_id, start_date, end_date).await?;
        let returning_readers = (total_readers as f64 * 0.3) as i64; // 假设30%是回访读者
        let new_readers = total_readers - returning_readers;

        Ok(AudienceAnalytics {
            total_readers,
            returning_readers,
            new_readers,
            avg_session_duration: 180.0, // 假设平均3分钟
            top_referrers: vec![
                ReferrerInfo {
                    source: "Direct".to_string(),
                    count: (total_readers as f64 * 0.4) as i64,
                    percentage: 40.0,
                },
                ReferrerInfo {
                    source: "Google".to_string(),
                    count: (total_readers as f64 * 0.3) as i64,
                    percentage: 30.0,
                },
                ReferrerInfo {
                    source: "Social Media".to_string(),
                    count: (total_readers as f64 * 0.2) as i64,
                    percentage: 20.0,
                },
                ReferrerInfo {
                    source: "Other".to_string(),
                    count: (total_readers as f64 * 0.1) as i64,
                    percentage: 10.0,
                },
            ],
            device_breakdown: DeviceBreakdown {
                desktop: (total_readers as f64 * 0.6) as i64,
                mobile: (total_readers as f64 * 0.35) as i64,
                tablet: (total_readers as f64 * 0.05) as i64,
            },
            geographic_distribution: vec![
                GeographicInfo {
                    country: "United States".to_string(),
                    count: (total_readers as f64 * 0.4) as i64,
                    percentage: 40.0,
                },
                GeographicInfo {
                    country: "United Kingdom".to_string(),
                    count: (total_readers as f64 * 0.2) as i64,
                    percentage: 20.0,
                },
                GeographicInfo {
                    country: "China".to_string(),
                    count: (total_readers as f64 * 0.15) as i64,
                    percentage: 15.0,
                },
                GeographicInfo {
                    country: "Others".to_string(),
                    count: (total_readers as f64 * 0.25) as i64,
                    percentage: 25.0,
                },
            ],
        })
    }

    /// 获取标签分析
    pub async fn get_top_tags_analytics(
        &self,
        user_id: &str,
        limit: i32,
    ) -> Result<Vec<TagAnalytics>> {
        let query = r#"
            SELECT 
                t.id as tag_id,
                t.name,
                COUNT(DISTINCT at.article_id) as total_articles,
                SUM(a.view_count) as total_views,
                SUM(a.clap_count) as total_claps,
                AVG((a.clap_count + a.comment_count + a.bookmark_count) * 100.0 / NULLIF(a.view_count, 0)) as avg_engagement
            FROM tag t
            JOIN article_tag at ON t.id = at.tag_id
            JOIN article a ON at.article_id = a.id
            WHERE a.author_id = $user_id
            AND a.status = 'published'
            GROUP BY t.id, t.name
            ORDER BY total_views DESC
            LIMIT $limit
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "user_id": user_id,
            "limit": limit
        })).await?;

        let tags: Vec<Value> = response.take(0)?;
        let mut results = Vec::new();

        for tag_data in tags {
            results.push(TagAnalytics {
                tag_id: tag_data["tag_id"].as_str().unwrap_or("").to_string(),
                name: tag_data["name"].as_str().unwrap_or("").to_string(),
                total_articles: tag_data["total_articles"].as_i64().unwrap_or(0),
                total_views: tag_data["total_views"].as_i64().unwrap_or(0),
                total_claps: tag_data["total_claps"].as_i64().unwrap_or(0),
                avg_engagement: tag_data["avg_engagement"].as_f64().unwrap_or(0.0),
                growth_rate: 0.0, // TODO: 实现增长率计算
            });
        }

        Ok(results)
    }

    /// 获取内容表现分析
    pub async fn get_content_performance(&self, user_id: &str) -> Result<ContentPerformance> {
        // 获取表现最好的文章
        let best_query = r#"
            SELECT * FROM (
                SELECT 
                    id as article_id,
                    title,
                    slug,
                    view_count as views,
                    clap_count as claps,
                    comment_count as comments,
                    bookmark_count as bookmarks,
                    share_count as shares,
                    reading_time as avg_read_time,
                    published_at,
                    (clap_count + comment_count + bookmark_count) * 100.0 / NULLIF(view_count, 0) as engagement_rate
                FROM article
                WHERE author_id = $user_id
                AND status = 'published'
                AND view_count > 100
            )
            ORDER BY engagement_rate DESC
            LIMIT 5
        "#;

        let mut best_response = self.db.query_with_params(best_query, json!({
            "user_id": user_id
        })).await?;
        let best_articles = self.parse_article_analytics(best_response.take(0)?).await?;

        // 获取表现不佳的文章
        let worst_query = r#"
            SELECT * FROM (
                SELECT 
                    id as article_id,
                    title,
                    slug,
                    view_count as views,
                    clap_count as claps,
                    comment_count as comments,
                    bookmark_count as bookmarks,
                    share_count as shares,
                    reading_time as avg_read_time,
                    published_at,
                    (clap_count + comment_count + bookmark_count) * 100.0 / NULLIF(view_count, 0) as engagement_rate
                FROM article
                WHERE author_id = $user_id
                AND status = 'published'
                AND published_at < $week_ago
                AND view_count < 50
            )
            ORDER BY engagement_rate ASC
            LIMIT 5
        "#;

        let week_ago = Utc::now() - Duration::days(7);
        let mut worst_response = self.db.query_with_params(worst_query, json!({
            "user_id": user_id,
            "week_ago": week_ago
        })).await?;
        let underperforming_articles = self.parse_article_analytics(worst_response.take(0)?).await?;

        // 获取最佳发布时间
        let optimal_times = self.get_optimal_publish_times(user_id).await?;

        // 生成内容建议
        let suggestions = self.generate_content_suggestions(user_id).await?;

        Ok(ContentPerformance {
            best_performing_articles: best_articles,
            underperforming_articles,
            optimal_publish_times: optimal_times,
            content_suggestions: suggestions,
        })
    }

    /// 获取趋势分析
    pub async fn get_trend_analytics(
        &self,
        user_id: &str,
        start_date: &DateTime<Utc>,
        end_date: &DateTime<Utc>,
    ) -> Result<TrendAnalytics> {
        let time_series = self.get_time_series_data(user_id, start_date, end_date, Some(AnalyticsPeriod::Day)).await?;
        
        let mut metrics = Vec::new();
        let mut peak_value = 0i64;
        let mut peak_day = Utc::now();

        for item in &time_series {
            let total_engagement = item.views + item.claps + item.comments + item.bookmarks;
            if total_engagement > peak_value {
                peak_value = total_engagement;
                peak_day = item.date;
            }

            metrics.push(TrendDataPoint {
                date: item.date,
                value: total_engagement,
                label: format!("{}", item.date.format("%Y-%m-%d")),
            });
        }

        // 计算增长率
        let first_value = metrics.first().map(|m| m.value).unwrap_or(0) as f64;
        let last_value = metrics.last().map(|m| m.value).unwrap_or(0) as f64;
        let growth_percentage = if first_value > 0.0 {
            ((last_value - first_value) / first_value) * 100.0
        } else {
            0.0
        };

        Ok(TrendAnalytics {
            period: "daily".to_string(),
            metrics,
            growth_percentage,
            peak_day,
            peak_value,
        })
    }

    /// 获取收入分析（如果有付费内容）
    pub async fn get_revenue_analytics(&self, user_id: &str) -> Result<RevenueAnalytics> {
        // 这是一个占位实现
        Ok(RevenueAnalytics {
            total_revenue: 0.0,
            paid_subscribers: 0,
            conversion_rate: 0.0,
            avg_revenue_per_user: 0.0,
            monthly_recurring_revenue: 0.0,
            churn_rate: 0.0,
        })
    }

    /// 获取实时分析
    pub async fn get_realtime_analytics(&self, user_id: &str) -> Result<RealtimeAnalytics> {
        // 获取当前活跃读者数
        let active_readers = self.get_active_readers_count(user_id).await?;
        
        // 获取正在被阅读的文章
        let articles_being_read = self.get_articles_being_read(user_id, 5).await?;
        
        // 获取最近的互动
        let recent_interactions = self.get_recent_interactions(user_id, 10).await?;

        Ok(RealtimeAnalytics {
            active_readers,
            articles_being_read,
            recent_interactions,
        })
    }

    // Helper methods

    async fn get_follower_count(&self, user_id: &str) -> Result<i64> {
        let query = "SELECT COUNT(*) as count FROM follow WHERE following_id = $user_id";
        let mut response = self.db.query_with_params(query, json!({"user_id": user_id})).await?;
        let result: Vec<Value> = response.take(0)?;
        Ok(result.first()
            .and_then(|v| v["count"].as_i64())
            .unwrap_or(0))
    }

    async fn get_following_count(&self, user_id: &str) -> Result<i64> {
        let query = "SELECT COUNT(*) as count FROM follow WHERE follower_id = $user_id";
        let mut response = self.db.query_with_params(query, json!({"user_id": user_id})).await?;
        let result: Vec<Value> = response.take(0)?;
        Ok(result.first()
            .and_then(|v| v["count"].as_i64())
            .unwrap_or(0))
    }

    async fn get_unique_viewers_count(&self, article_id: &str) -> Result<i64> {
        // 简化实现，实际应该有用户访问记录
        let query = "SELECT view_count FROM article WHERE id = $article_id";
        let mut response = self.db.query_with_params(query, json!({"article_id": article_id})).await?;
        let result: Vec<Value> = response.take(0)?;
        let total_views = result.first()
            .and_then(|v| v["view_count"].as_i64())
            .unwrap_or(0);
        // 假设唯一查看者约为总查看数的70%
        Ok((total_views as f64 * 0.7) as i64)
    }

    async fn get_total_readers_count(
        &self,
        user_id: &str,
        start_date: &DateTime<Utc>,
        end_date: &DateTime<Utc>,
    ) -> Result<i64> {
        let query = r#"
            SELECT SUM(view_count) as total_views
            FROM article
            WHERE author_id = $user_id
            AND published_at >= $start_date
            AND published_at <= $end_date
            AND status = 'published'
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "user_id": user_id,
            "start_date": start_date,
            "end_date": end_date
        })).await?;

        let result: Vec<Value> = response.take(0)?;
        let total_views = result.first()
            .and_then(|v| v["total_views"].as_i64())
            .unwrap_or(0);

        // 假设每个读者平均查看2篇文章
        Ok((total_views as f64 / 2.0) as i64)
    }

    async fn get_optimal_publish_times(&self, user_id: &str) -> Result<Vec<OptimalTimeSlot>> {
        // 简化实现，返回一些常见的最佳发布时间
        Ok(vec![
            OptimalTimeSlot {
                day_of_week: "Monday".to_string(),
                hour: 9,
                avg_engagement: 85.5,
            },
            OptimalTimeSlot {
                day_of_week: "Tuesday".to_string(),
                hour: 10,
                avg_engagement: 82.3,
            },
            OptimalTimeSlot {
                day_of_week: "Wednesday".to_string(),
                hour: 14,
                avg_engagement: 79.8,
            },
            OptimalTimeSlot {
                day_of_week: "Thursday".to_string(),
                hour: 11,
                avg_engagement: 81.2,
            },
            OptimalTimeSlot {
                day_of_week: "Friday".to_string(),
                hour: 8,
                avg_engagement: 77.5,
            },
        ])
    }

    async fn generate_content_suggestions(&self, user_id: &str) -> Result<Vec<ContentSuggestion>> {
        let mut suggestions = Vec::new();

        // 获取用户最近的文章统计
        let recent_stats = self.get_user_overview(user_id).await?;

        // 基于统计生成建议
        if recent_stats.avg_reading_time < 3.0 {
            suggestions.push(ContentSuggestion {
                suggestion_type: SuggestionType::Length,
                message: "Consider writing longer, more in-depth articles. Your average reading time is under 3 minutes.".to_string(),
                priority: Priority::Medium,
            });
        }

        if recent_stats.engagement_rate < 5.0 {
            suggestions.push(ContentSuggestion {
                suggestion_type: SuggestionType::Topic,
                message: "Try exploring trending topics in your niche to increase engagement.".to_string(),
                priority: Priority::High,
            });
        }

        if recent_stats.total_articles < 5 {
            suggestions.push(ContentSuggestion {
                suggestion_type: SuggestionType::SeriesCreation,
                message: "Consider creating a series of related articles to build reader loyalty.".to_string(),
                priority: Priority::Low,
            });
        }

        Ok(suggestions)
    }

    async fn parse_article_analytics(&self, articles: Vec<Value>) -> Result<Vec<ArticleAnalytics>> {
        let mut results = Vec::new();

        for article_data in articles {
            let article_id = article_data["article_id"].as_str().unwrap_or("");
            let unique_viewers = self.get_unique_viewers_count(article_id).await?;

            results.push(ArticleAnalytics {
                article_id: article_id.to_string(),
                title: article_data["title"].as_str().unwrap_or("").to_string(),
                slug: article_data["slug"].as_str().unwrap_or("").to_string(),
                views: article_data["views"].as_i64().unwrap_or(0),
                unique_viewers,
                claps: article_data["claps"].as_i64().unwrap_or(0),
                comments: article_data["comments"].as_i64().unwrap_or(0),
                bookmarks: article_data["bookmarks"].as_i64().unwrap_or(0),
                shares: article_data["shares"].as_i64().unwrap_or(0),
                avg_read_time: article_data["avg_read_time"].as_f64().unwrap_or(0.0),
                bounce_rate: 0.0,
                engagement_rate: article_data["engagement_rate"].as_f64().unwrap_or(0.0),
                published_at: article_data["published_at"]
                    .as_str()
                    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(Utc::now),
            });
        }

        Ok(results)
    }

    async fn get_active_readers_count(&self, user_id: &str) -> Result<i64> {
        // 简化实现，返回一个模拟值
        // 实际应该根据会话或WebSocket连接统计
        Ok(5)
    }

    async fn get_articles_being_read(&self, user_id: &str, limit: i32) -> Result<Vec<ArticleReadInfo>> {
        // 简化实现，返回最近发布的文章
        let query = r#"
            SELECT id as article_id, title
            FROM article
            WHERE author_id = $user_id
            AND status = 'published'
            ORDER BY published_at DESC
            LIMIT $limit
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "user_id": user_id,
            "limit": limit
        })).await?;

        let articles: Vec<Value> = response.take(0)?;
        let mut results = Vec::new();

        for (idx, article) in articles.iter().enumerate() {
            results.push(ArticleReadInfo {
                article_id: article["article_id"].as_str().unwrap_or("").to_string(),
                title: article["title"].as_str().unwrap_or("").to_string(),
                reader_count: (5 - idx) as i64, // 模拟递减的读者数
            });
        }

        Ok(results)
    }

    async fn get_recent_interactions(&self, user_id: &str, limit: i32) -> Result<Vec<InteractionInfo>> {
        // 简化实现，返回一些模拟的最近互动
        let query = r#"
            SELECT 
                'clap' as interaction_type,
                a.id as article_id,
                a.title as article_title,
                c.created_at as timestamp
            FROM clap c
            JOIN article a ON c.article_id = a.id
            WHERE a.author_id = $user_id
            ORDER BY c.created_at DESC
            LIMIT $limit
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "user_id": user_id,
            "limit": limit
        })).await?;

        let interactions: Vec<Value> = response.take(0)?;
        let mut results = Vec::new();

        for interaction in interactions {
            results.push(InteractionInfo {
                interaction_type: interaction["interaction_type"].as_str().unwrap_or("").to_string(),
                article_id: interaction["article_id"].as_str().unwrap_or("").to_string(),
                article_title: interaction["article_title"].as_str().unwrap_or("").to_string(),
                user_name: None, // 隐私保护，不显示具体用户
                timestamp: interaction["timestamp"]
                    .as_str()
                    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(Utc::now),
            });
        }

        Ok(results)
    }

    /// 导出分析数据
    pub async fn export_analytics(
        &self,
        user_id: &str,
        options: ExportOptions,
    ) -> Result<Vec<u8>> {
        debug!("Exporting analytics for user: {} with format: {:?}", user_id, options.format);

        match options.format {
            ExportFormat::Json => {
                let dashboard = self.get_user_dashboard(user_id, AnalyticsQuery {
                    start_date: options.date_range.map(|(start, _)| start),
                    end_date: options.date_range.map(|(_, end)| end),
                    period: None,
                    metric: None,
                    limit: None,
                }).await?;

                let json_data = serde_json::to_vec_pretty(&dashboard)?;
                Ok(json_data)
            }
            ExportFormat::Csv => {
                // 简化的CSV导出
                let mut csv_data = String::from("Date,Views,Claps,Comments,Bookmarks\n");
                
                let time_series = self.get_time_series_data(
                    user_id,
                    &options.date_range.map(|(start, _)| start).unwrap_or(Utc::now() - Duration::days(30)),
                    &options.date_range.map(|(_, end)| end).unwrap_or(Utc::now()),
                    Some(AnalyticsPeriod::Day),
                ).await?;

                for item in time_series {
                    csv_data.push_str(&format!(
                        "{},{},{},{},{}\n",
                        item.date.format("%Y-%m-%d"),
                        item.views,
                        item.claps,
                        item.comments,
                        item.bookmarks
                    ));
                }

                Ok(csv_data.into_bytes())
            }
            _ => Err(AppError::BadRequest("Export format not supported yet".to_string())),
        }
    }
}