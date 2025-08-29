use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use validator::Validate;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicationMember {
    pub id: String,
    pub publication_id: String,
    pub user_id: String,
    pub role: MemberRole,
    pub permissions: Vec<String>,
    pub joined_at: DateTime<Utc>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MemberRole {
    Owner,
    Editor,
    Writer,
    Contributor,
}

impl MemberRole {
    pub fn default_permissions(&self) -> Vec<String> {
        match self {
            MemberRole::Owner => vec![
                "publication.read".to_string(),
                "publication.write".to_string(),
                "publication.delete".to_string(),
                "publication.manage_members".to_string(),
                "publication.manage_settings".to_string(),
                "article.create".to_string(),
                "article.publish".to_string(),
                "article.edit_any".to_string(),
                "article.delete_any".to_string(),
            ],
            MemberRole::Editor => vec![
                "publication.read".to_string(),
                "publication.write".to_string(),
                "publication.manage_members".to_string(),
                "article.create".to_string(),
                "article.publish".to_string(),
                "article.edit_any".to_string(),
            ],
            MemberRole::Writer => vec![
                "publication.read".to_string(),
                "article.create".to_string(),
                "article.publish".to_string(),
                "article.edit_own".to_string(),
            ],
            MemberRole::Contributor => vec![
                "publication.read".to_string(),
                "article.create".to_string(),
                "article.edit_own".to_string(),
            ],
        }
    }

    pub fn can_approve_submissions(&self) -> bool {
        matches!(self, MemberRole::Owner | MemberRole::Editor)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicationSubmission {
    pub id: String,
    pub publication_id: String,
    pub article_id: String,
    pub author_id: String,
    pub status: SubmissionStatus,
    pub submitted_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub reviewed_by: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubmissionStatus {
    Pending,
    Approved,
    Rejected,
    Withdrawn,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicationResponse {
    #[serde(flatten)]
    pub publication: Publication,
    pub is_member: bool,
    pub member_role: Option<MemberRole>,
    pub is_following: bool,
    pub recent_articles: Vec<crate::models::article::ArticleListItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicationFollow {
    pub id: String,
    pub user_id: String,
    pub publication_id: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicationListItem {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub tagline: Option<String>,
    pub logo_url: Option<String>,
    pub cover_image_url: Option<String>,
    pub member_count: i64,
    pub article_count: i64,
    pub follower_count: i64,
    pub is_verified: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct CreatePublicationRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    
    #[validate(length(max = 500))]
    pub description: Option<String>,
    
    #[validate(length(max = 100))]
    pub tagline: Option<String>,
    
    #[validate(url)]
    pub logo_url: Option<String>,
    
    #[validate(url)]
    pub cover_image_url: Option<String>,
    
    pub homepage_layout: Option<String>,
    pub theme_color: Option<String>,
    
    #[validate(url)]
    pub custom_domain: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct UpdatePublicationRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: Option<String>,
    
    #[validate(length(max = 500))]
    pub description: Option<String>,
    
    #[validate(length(max = 100))]
    pub tagline: Option<String>,
    
    #[validate(url)]
    pub logo_url: Option<String>,
    
    #[validate(url)]
    pub cover_image_url: Option<String>,
    
    pub homepage_layout: Option<String>,
    pub theme_color: Option<String>,
    
    #[validate(url)]
    pub custom_domain: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct AddMemberRequest {
    #[validate(length(min = 1))]
    pub user_id: String,
    
    pub role: MemberRole,
    pub permissions: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct UpdateMemberRequest {
    pub role: Option<MemberRole>,
    pub permissions: Option<Vec<String>>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct SubmitArticleRequest {
    #[validate(length(min = 1))]
    pub article_id: String,
    
    #[validate(length(max = 500))]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ReviewSubmissionRequest {
    pub status: SubmissionStatus,
    
    #[validate(length(max = 500))]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PublicationQuery {
    pub page: Option<usize>,
    pub limit: Option<usize>,
    pub search: Option<String>,
    pub verified_only: Option<bool>,
    pub sort: Option<String>, // "newest", "oldest", "popular", "alphabetical"
}

impl Default for PublicationQuery {
    fn default() -> Self {
        Self {
            page: Some(1),
            limit: Some(20),
            search: None,
            verified_only: None,
            sort: Some("popular".to_string()),
        }
    }
}