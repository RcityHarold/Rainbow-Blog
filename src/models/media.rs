use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use surrealdb::sql::Thing;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaFile {
    pub id: Thing,
    pub user_id: String,
    pub filename: String,
    pub original_filename: String,
    pub content_type: String,
    pub size: i64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub storage_path: String,
    pub public_url: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MediaUploadResponse {
    pub id: String,
    pub url: String,
    pub filename: String,
    pub size: i64,
    pub content_type: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MediaStats {
    pub total_files: i64,
    pub total_size: i64,
    pub images_count: i64,
    pub user_storage_used: i64,
}

impl MediaFile {
    pub fn to_response(&self) -> MediaUploadResponse {
        MediaUploadResponse {
            id: self.id.id.to_string(),
            url: self.public_url.clone(),
            filename: self.filename.clone(),
            size: self.size,
            content_type: self.content_type.clone(),
            width: self.width,
            height: self.height,
        }
    }
}