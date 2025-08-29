use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Series {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub slug: String,
    pub author_id: String,
    pub cover_image_url: Option<String>,
    pub article_count: i64,
    pub is_completed: bool,
    pub is_public: bool,
    pub view_count: i64,
    pub subscriber_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesArticle {
    pub id: String,
    pub series_id: String,
    pub article_id: String,
    pub order_index: i32,
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesResponse {
    #[serde(flatten)]
    pub series: Series,
    pub author_name: String,
    pub author_username: String,
    pub author_avatar: Option<String>,
    pub is_subscribed: bool,
    pub articles: Vec<SeriesArticleInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesArticleInfo {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub slug: String,
    pub excerpt: Option<String>,
    pub cover_image_url: Option<String>,
    pub reading_time: i32,
    pub order_index: i32,
    pub is_published: bool,
    pub published_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesListItem {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub description: Option<String>,
    pub cover_image_url: Option<String>,
    pub author_id: String,
    pub author_name: String,
    pub author_username: String,
    pub article_count: i64,
    pub is_completed: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesSubscription {
    pub id: String,
    pub series_id: String,
    pub user_id: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct CreateSeriesRequest {
    #[validate(length(min = 1, max = 200))]
    pub title: String,
    
    #[validate(length(max = 1000))]
    pub description: Option<String>,
    
    #[validate(url)]
    pub cover_image_url: Option<String>,
    
    pub is_public: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct UpdateSeriesRequest {
    #[validate(length(min = 1, max = 200))]
    pub title: Option<String>,
    
    #[validate(length(max = 1000))]
    pub description: Option<String>,
    
    #[validate(url)]
    pub cover_image_url: Option<String>,
    
    pub is_public: Option<bool>,
    pub is_completed: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct AddArticleToSeriesRequest {
    #[validate(length(min = 1))]
    pub article_id: String,
    pub order_index: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct UpdateArticleOrderRequest {
    pub articles: Vec<ArticleOrderItem>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ArticleOrderItem {
    pub article_id: String,
    pub order_index: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SeriesQuery {
    pub author_id: Option<String>,
    pub is_completed: Option<bool>,
    pub is_public: Option<bool>,
    pub search: Option<String>,
    pub page: Option<usize>,
    pub limit: Option<usize>,
    pub sort: Option<String>, // "newest", "oldest", "popular", "alphabetical"
}

impl Default for SeriesQuery {
    fn default() -> Self {
        Self {
            author_id: None,
            is_completed: None,
            is_public: Some(true),
            search: None,
            page: Some(1),
            limit: Some(20),
            sort: Some("newest".to_string()),
        }
    }
}