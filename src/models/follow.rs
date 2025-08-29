use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Follow {
    pub id: String,
    pub follower_id: String,
    pub following_id: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowUserInfo {
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub is_verified: bool,
    pub article_count: i64,
    pub follower_count: i64,
    pub is_following: bool, // 当前用户是否关注了该用户
    pub is_followed_back: bool, // 该用户是否回关了当前用户
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowStats {
    pub followers_count: i64,
    pub following_count: i64,
    pub is_following: bool,
    pub is_followed_by: bool,
}