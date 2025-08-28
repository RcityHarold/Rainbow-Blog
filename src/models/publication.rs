use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Publication {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub tagline: Option<String>,
    pub logo_url: Option<String>,
    pub cover_image_url: Option<String>,
    pub owner_id: String,
    pub homepage_layout: String,
    pub theme_color: String,
    pub custom_domain: Option<String>,
    pub member_count: i64,
    pub article_count: i64,
    pub follower_count: i64,
    pub is_verified: bool,
    pub is_suspended: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}