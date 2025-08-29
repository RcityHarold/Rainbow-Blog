use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub recipient_id: String,
    pub notification_type: String,
    pub title: String,
    pub message: String,
    pub data: serde_json::Value,
    pub is_read: bool,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateNotificationRequest {
    pub recipient_id: String,
    pub notification_type: NotificationType,
    pub title: String,
    pub message: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationType {
    Follow,
    ArticlePublished,
    Comment,
    CommentReply,
    Clap,
    Mention,
}