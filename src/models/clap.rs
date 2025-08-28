use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clap {
    pub id: String,
    pub user_id: String,
    pub article_id: String,
    pub count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}