use serde::{Deserialize, Serialize};
use crate::models::{article::ArticleListItem, tag::Tag, user::UserProfile};
use chrono::{DateTime, Utc};
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub search_type: Option<SearchType>,
    pub page: Option<i32>,
    pub limit: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct AdvancedSearchQuery {
    pub q: Option<String>,
    pub search_type: Option<SearchType>,
    
    // Article filters
    pub author: Option<String>,
    pub tags: Option<Vec<String>>,
    pub publication: Option<String>,
    pub series: Option<String>,
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
    pub min_reading_time: Option<i32>,
    pub max_reading_time: Option<i32>,
    pub min_claps: Option<i64>,
    pub is_featured: Option<bool>,
    pub has_audio: Option<bool>,
    pub is_paid: Option<bool>,
    
    // Sorting
    pub sort_by: Option<SortBy>,
    pub sort_order: Option<SortOrder>,
    
    // Pagination
    pub page: Option<i32>,
    pub limit: Option<i32>,
    
    // Advanced options
    pub include_drafts: Option<bool>, // Only for author's own articles
    pub language: Option<String>,
    pub exclude_read: Option<bool>, // For logged-in users
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchType {
    All,
    Articles,
    Users,
    Tags,
    Publications,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResults {
    pub articles: Vec<ArticleSearchResult>,
    pub users: Vec<UserSearchResult>,
    pub tags: Vec<TagSearchResult>,
    pub publications: Vec<PublicationSearchResult>,
    pub total_results: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleSearchResult {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub excerpt: Option<String>,
    pub author_name: String,
    pub author_username: String,
    pub cover_image_url: Option<String>,
    pub reading_time: i32,
    pub published_at: chrono::DateTime<chrono::Utc>,
    pub clap_count: i64,
    pub comment_count: i64,
    pub tags: Vec<String>,
    pub highlight: Option<SearchHighlight>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSearchResult {
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub is_verified: bool,
    pub follower_count: i64,
    pub article_count: i64,
    pub highlight: Option<SearchHighlight>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagSearchResult {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub article_count: i64,
    pub follower_count: i64,
    pub is_featured: bool,
    pub highlight: Option<SearchHighlight>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicationSearchResult {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub tagline: Option<String>,
    pub logo_url: Option<String>,
    pub member_count: i64,
    pub article_count: i64,
    pub follower_count: i64,
    pub highlight: Option<SearchHighlight>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHighlight {
    pub field: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchSuggestion {
    pub text: String,
    pub suggestion_type: SuggestionType,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SuggestionType {
    Query,
    Tag,
    User,
    Publication,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchIndex {
    pub article_id: String,
    pub title: String,
    pub content: String,
    pub author_name: String,
    pub tags: Vec<String>,
    pub publication_name: Option<String>,
    pub is_published: bool,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
    pub popularity_score: f64,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortBy {
    Relevance,
    PublishedAt,
    UpdatedAt,
    ClapCount,
    CommentCount,
    ViewCount,
    ReadingTime,
    Title,
    AuthorName,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedSearchResults {
    pub articles: Vec<ArticleSearchResult>,
    pub users: Vec<UserSearchResult>,
    pub tags: Vec<TagSearchResult>,
    pub publications: Vec<PublicationSearchResult>,
    pub series: Vec<SeriesSearchResult>,
    pub total_results: i64,
    pub page: i32,
    pub total_pages: i32,
    pub facets: SearchFacets,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesSearchResult {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub description: Option<String>,
    pub author_name: String,
    pub author_username: String,
    pub article_count: i64,
    pub is_completed: bool,
    pub created_at: DateTime<Utc>,
    pub highlight: Option<SearchHighlight>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchFacets {
    pub tags: Vec<FacetItem>,
    pub authors: Vec<FacetItem>,
    pub publications: Vec<FacetItem>,
    pub date_ranges: Vec<DateRangeFacet>,
    pub reading_time_ranges: Vec<RangeFacet>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacetItem {
    pub value: String,
    pub label: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRangeFacet {
    pub label: String,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RangeFacet {
    pub label: String,
    pub min: i32,
    pub max: i32,
    pub count: i64,
}