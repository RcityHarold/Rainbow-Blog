use crate::{
    error::{AppError, Result},
    models::{bookmark::*, article::Article},
    services::Database,
};
use chrono::Utc;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, info};
use validator::Validate;
use uuid::Uuid;

#[derive(Clone)]
pub struct BookmarkService {
    db: Arc<Database>,
}

impl BookmarkService {
    pub async fn new(db: Arc<Database>) -> Result<Self> {
        Ok(Self { db })
    }

    pub async fn create_bookmark(
        &self,
        user_id: &str,
        request: CreateBookmarkRequest,
    ) -> Result<Bookmark> {
        debug!("Creating bookmark for article: {} by user: {}", request.article_id, user_id);

        request
            .validate()
            .map_err(|e| AppError::ValidatorError(e))?;

        // Verify article exists and is published
        let article: Article = self
            .db
            .get_by_id("article", &request.article_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Article not found".to_string()))?;

        if article.status != crate::models::article::ArticleStatus::Published {
            return Err(AppError::forbidden(
                "Cannot bookmark unpublished articles",
            ));
        }

        // Check if bookmark already exists
        let query = r#"
            SELECT * FROM bookmark 
            WHERE user_id = $user_id 
            AND article_id = $article_id
        "#;
        
        let mut response = self.db.query_with_params(query, json!({
            "user_id": user_id,
            "article_id": &request.article_id
        })).await?;
        
        let existing: Vec<Bookmark> = response.take(0)?;

        if !existing.is_empty() {
            return Err(AppError::Conflict(
                "Article is already bookmarked".to_string(),
            ));
        }

        let bookmark = Bookmark {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            article_id: request.article_id,
            note: request.note,
            created_at: Utc::now(),
        };

        let created: Bookmark = self.db.create("bookmark", bookmark).await?;

        // Update article bookmark count
        self.update_article_bookmark_count(&created.article_id).await?;

        Ok(created)
    }

    pub async fn get_user_bookmarks(
        &self,
        user_id: &str,
        page: Option<i32>,
        limit: Option<i32>,
    ) -> Result<Vec<BookmarkWithArticle>> {
        debug!("Getting bookmarks for user: {}", user_id);

        let page = page.unwrap_or(1).max(1);
        let limit = limit.unwrap_or(20).min(100);
        let offset = (page - 1) * limit;

        let query = r#"
            SELECT 
                b.*,
                a.title as article_title,
                a.slug as article_slug,
                a.excerpt as article_excerpt,
                a.cover_image_url as article_cover_image,
                a.reading_time as article_reading_time,
                u.display_name as author_name,
                u.username as author_username
            FROM bookmark b
            JOIN article a ON b.article_id = a.id
            JOIN user_profile u ON a.author_id = u.user_id
            WHERE b.user_id = $user_id
            AND a.is_deleted = false
            ORDER BY b.created_at DESC
            LIMIT $limit
            START $offset
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "user_id": user_id,
            "limit": limit,
            "offset": offset
        })).await?;
        
        let bookmarks: Vec<Value> = response.take(0)?;

        let mut result = Vec::new();
        for bookmark_data in bookmarks {
            if let Ok(bookmark_with_article) = serde_json::from_value::<BookmarkWithArticle>(bookmark_data) {
                result.push(bookmark_with_article);
            }
        }

        Ok(result)
    }

    pub async fn get_bookmark(&self, bookmark_id: &str, user_id: &str) -> Result<Option<Bookmark>> {
        let bookmark: Option<Bookmark> = self.db.get_by_id("bookmark", bookmark_id).await?;

        if let Some(ref b) = bookmark {
            if b.user_id != user_id {
                return Ok(None);
            }
        }

        Ok(bookmark)
    }

    pub async fn update_bookmark(
        &self,
        bookmark_id: &str,
        user_id: &str,
        request: UpdateBookmarkRequest,
    ) -> Result<Bookmark> {
        request
            .validate()
            .map_err(|e| AppError::ValidatorError(e))?;

        let bookmark: Bookmark = self
            .db
            .get_by_id("bookmark", bookmark_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Bookmark not found".to_string()))?;

        if bookmark.user_id != user_id {
            return Err(AppError::forbidden(
                "You can only update your own bookmarks",
            ));
        }

        let updates = json!({
            "note": request.note,
        });

        let updated: Bookmark = self.db.update_by_id_with_json("bookmark", bookmark_id, updates).await?.ok_or_else(|| AppError::internal("Failed to update bookmark"))?;

        Ok(updated)
    }

    pub async fn delete_bookmark(&self, bookmark_id: &str, user_id: &str) -> Result<()> {
        let bookmark: Bookmark = self
            .db
            .get_by_id("bookmark", bookmark_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Bookmark not found".to_string()))?;

        if bookmark.user_id != user_id {
            return Err(AppError::forbidden(
                "You can only delete your own bookmarks",
            ));
        }

        self.db.delete_by_id("bookmark", bookmark_id).await?;

        // Update article bookmark count
        self.update_article_bookmark_count(&bookmark.article_id).await?;

        Ok(())
    }

    pub async fn delete_bookmark_by_article(
        &self,
        article_id: &str,
        user_id: &str,
    ) -> Result<()> {
        debug!("Deleting bookmark for article: {} by user: {}", article_id, user_id);

        let query = r#"
            DELETE bookmark 
            WHERE user_id = $user_id 
            AND article_id = $article_id
        "#;

        self.db.query_with_params(query, json!({
            "user_id": user_id,
            "article_id": article_id
        })).await?;

        // Update article bookmark count
        self.update_article_bookmark_count(article_id).await?;

        Ok(())
    }

    pub async fn is_bookmarked(&self, article_id: &str, user_id: &str) -> Result<bool> {
        let query = r#"
            SELECT count() as count 
            FROM bookmark 
            WHERE user_id = $user_id 
            AND article_id = $article_id
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "user_id": user_id,
            "article_id": article_id
        })).await?;
        
        let result: Vec<Value> = response.take(0)?;

        let count = result
            .first()
            .and_then(|v| v.get("count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        Ok(count > 0)
    }

    async fn update_article_bookmark_count(&self, article_id: &str) -> Result<()> {
        let query = r#"
            LET $count = (SELECT count() FROM bookmark WHERE article_id = $article_id);
            UPDATE article SET bookmark_count = $count WHERE id = $article_id;
        "#;

        self.db.query_with_params(query, json!({
            "article_id": article_id
        })).await?;

        Ok(())
    }
}