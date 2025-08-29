use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub follower_count: i64,
    pub article_count: i64,
    pub is_featured: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateTagRequest {
    #[validate(length(min = 1, max = 50))]
    pub name: String,
    #[validate(length(max = 200))]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UpdateTagRequest {
    #[validate(length(min = 1, max = 50))]
    pub name: Option<String>,
    #[validate(length(max = 200))]
    pub description: Option<String>,
    pub is_featured: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleTag {
    pub id: String,
    pub article_id: String,
    pub tag_id: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserTagFollow {
    pub id: String,
    pub user_id: String,
    pub tag_id: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagWithFollowStatus {
    #[serde(flatten)]
    pub tag: Tag,
    pub is_following: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagQuery {
    pub search: Option<String>,
    pub featured_only: Option<bool>,
    pub sort_by: Option<String>, // popular, name, created_at
    pub page: Option<i32>,
    pub limit: Option<i32>,
}