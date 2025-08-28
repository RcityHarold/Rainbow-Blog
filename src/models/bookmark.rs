use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: String,
    pub user_id: String,
    pub article_id: String,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}