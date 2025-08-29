use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: String,
    pub user_id: String,
    pub article_id: String,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateBookmarkRequest {
    pub article_id: String,
    #[validate(length(max = 1000))]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UpdateBookmarkRequest {
    #[validate(length(max = 1000))]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkWithArticle {
    #[serde(flatten)]
    pub bookmark: Bookmark,
    pub article_title: String,
    pub article_slug: String,
    pub article_excerpt: Option<String>,
    pub article_cover_image: Option<String>,
    pub article_reading_time: i32,
    pub author_name: String,
    pub author_username: String,
}