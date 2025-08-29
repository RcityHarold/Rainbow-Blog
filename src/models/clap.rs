use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clap {
    pub id: String,
    pub user_id: String,
    pub article_id: String,
    pub count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct AddClapRequest {
    pub article_id: String,
    #[validate(range(min = 1, max = 50))]
    pub count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClapResponse {
    pub user_clap_count: i32,
    pub total_claps: i64,
}