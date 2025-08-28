use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Follow {
    pub id: String,
    pub follower_id: String,
    pub following_id: String,
    pub created_at: DateTime<Utc>,
}