use crate::{
    error::{AppError, Result},
    models::comment::*,
    models::article::Article,
    services::Database,
};
use chrono::Utc;
use serde_json::{json, Value};
use std::sync::Arc;
use std::collections::HashMap;
use tracing::{debug, error, info};
use validator::Validate;
use uuid::Uuid;

#[derive(Clone)]
pub struct CommentService {
    db: Arc<Database>,
}

impl CommentService {
    pub async fn new(db: Arc<Database>) -> Result<Self> {
        Ok(Self { db })
    }

    pub async fn create_comment(
        &self,
        user_id: &str,
        request: CreateCommentRequest,
    ) -> Result<Comment> {
        debug!("Creating comment for article: {}", request.article_id);

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
                "Cannot comment on unpublished articles",
            ));
        }

        // Verify parent comment exists if provided
        if let Some(parent_id) = &request.parent_id {
            let parent: Option<Comment> = self.db.get_by_id("comment", parent_id).await?;
            if parent.is_none() {
                return Err(AppError::NotFound("Parent comment not found".to_string()));
            }
        }

        // Check if this is an author response
        let is_author_response = article.author_id == user_id;

        let comment = Comment {
            id: Uuid::new_v4().to_string(),
            article_id: request.article_id.clone(),
            author_id: user_id.to_string(),
            parent_id: request.parent_id,
            content: request.content,
            is_author_response,
            clap_count: 0,
            is_edited: false,
            is_deleted: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            deleted_at: None,
        };

        let created: Comment = self.db.create("comment", comment).await?;

        // Update article comment count
        self.update_article_comment_count(&request.article_id).await?;

        Ok(created)
    }

    pub async fn get_comment(&self, comment_id: &str) -> Result<Option<Comment>> {
        let comment: Option<Comment> = self.db.get_by_id("comment", comment_id).await?;
        
        if let Some(ref c) = comment {
            if c.is_deleted {
                return Ok(None);
            }
        }
        
        Ok(comment)
    }

    pub async fn get_article_comments(
        &self,
        article_id: &str,
        user_id: Option<&str>,
    ) -> Result<Vec<CommentWithAuthor>> {
        debug!("Getting comments for article: {}", article_id);

        let query = r#"
            SELECT * FROM comment 
            WHERE article_id = $article_id 
            AND is_deleted = false 
            ORDER BY created_at DESC
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "article_id": article_id
        })).await?;
        let comments: Vec<Comment> = response.take(0)?;

        // Build comment tree
        let mut comment_tree = self.build_comment_tree(comments, user_id).await?;
        
        Ok(comment_tree)
    }

    pub async fn update_comment(
        &self,
        comment_id: &str,
        user_id: &str,
        request: UpdateCommentRequest,
    ) -> Result<Comment> {
        request
            .validate()
            .map_err(|e| AppError::ValidatorError(e))?;

        let comment: Comment = self
            .db
            .get_by_id("comment", comment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Comment not found".to_string()))?;

        if comment.is_deleted {
            return Err(AppError::NotFound("Comment not found".to_string()));
        }

        if comment.author_id != user_id {
            return Err(AppError::forbidden(
                "You can only edit your own comments",
            ));
        }

        let updates = json!({
            "content": request.content,
            "is_edited": true,
            "updated_at": Utc::now(),
        });

        let updated: Comment = self.db.update_by_id_with_json("comment", comment_id, updates).await?.ok_or_else(|| AppError::internal("Failed to update comment"))?;
        
        Ok(updated)
    }

    pub async fn delete_comment(&self, comment_id: &str, user_id: &str) -> Result<()> {
        let comment: Comment = self
            .db
            .get_by_id("comment", comment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Comment not found".to_string()))?;

        if comment.is_deleted {
            return Err(AppError::NotFound("Comment not found".to_string()));
        }

        if comment.author_id != user_id {
            return Err(AppError::forbidden(
                "You can only delete your own comments",
            ));
        }

        let updates = json!({
            "is_deleted": true,
            "deleted_at": Utc::now(),
        });

        self.db.update_by_id_with_json::<Value>("comment", comment_id, updates).await?;

        // Update article comment count
        self.update_article_comment_count(&comment.article_id).await?;

        Ok(())
    }

    pub async fn clap_comment(&self, comment_id: &str, user_id: &str) -> Result<()> {
        let comment: Comment = self
            .db
            .get_by_id("comment", comment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Comment not found".to_string()))?;

        if comment.is_deleted {
            return Err(AppError::NotFound("Comment not found".to_string()));
        }

        // Check if user has already clapped
        let query = r#"
            SELECT * FROM comment_clap 
            WHERE user_id = $user_id 
            AND comment_id = $comment_id
        "#;
        
        let mut response = self.db.query_with_params(query, json!({
            "user_id": user_id,
            "comment_id": comment_id
        })).await?;
        
        let existing_claps: Vec<CommentClap> = response.take(0)?;
        
        if !existing_claps.is_empty() {
            return Err(AppError::Conflict(
                "You have already clapped this comment".to_string(),
            ));
        }

        // Create clap
        let clap = CommentClap {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            comment_id: comment_id.to_string(),
            created_at: Utc::now(),
        };

        self.db.create("comment_clap", clap).await?;

        // Update clap count
        self.update_comment_clap_count(comment_id).await?;

        Ok(())
    }

    pub async fn remove_clap(&self, comment_id: &str, user_id: &str) -> Result<()> {
        let query = r#"
            DELETE comment_clap 
            WHERE user_id = $user_id 
            AND comment_id = $comment_id
        "#;

        self.db.query_with_params(query, json!({
            "user_id": user_id,
            "comment_id": comment_id
        })).await?;

        // Update clap count
        self.update_comment_clap_count(comment_id).await?;

        Ok(())
    }

    // Helper methods
    async fn build_comment_tree(
        &self,
        comments: Vec<Comment>,
        user_id: Option<&str>,
    ) -> Result<Vec<CommentWithAuthor>> {
        let mut comment_map: HashMap<String, CommentWithAuthor> = HashMap::new();
        let mut root_comments = Vec::new();

        // Get all author information
        let author_ids: Vec<&str> = comments.iter().map(|c| c.author_id.as_str()).collect();
        let authors = self.get_authors_info(&author_ids).await?;

        // Get user claps if user is authenticated
        let user_claps = if let Some(uid) = user_id {
            self.get_user_comment_claps(uid, &comments).await?
        } else {
            HashMap::new()
        };

        // Build comment nodes
        for comment in comments {
            let author_info = authors.get(&comment.author_id).cloned().unwrap_or_default();
            let user_has_clapped = user_claps.get(&comment.id).copied().unwrap_or(false);

            let comment_with_author = CommentWithAuthor {
                comment: comment.clone(),
                author_name: author_info.0,
                author_username: author_info.1,
                author_avatar: author_info.2,
                user_has_clapped,
                replies: Vec::new(),
            };

            comment_map.insert(comment.id.clone(), comment_with_author);
        }

        // Build tree structure
        let mut comment_map_clone = comment_map.clone();
        for (id, mut comment) in comment_map {
            if let Some(parent_id) = &comment.comment.parent_id {
                if let Some(parent) = comment_map_clone.get_mut(parent_id) {
                    parent.replies.push(comment);
                }
            } else {
                root_comments.push(comment);
            }
        }

        // Sort by creation date
        root_comments.sort_by(|a, b| b.comment.created_at.cmp(&a.comment.created_at));

        Ok(root_comments)
    }

    async fn get_authors_info(
        &self,
        author_ids: &[&str],
    ) -> Result<HashMap<String, (String, String, Option<String>)>> {
        let mut authors = HashMap::new();
        
        for id in author_ids {
            let query = r#"
                SELECT display_name, username, avatar_url 
                FROM user_profile 
                WHERE user_id = $user_id
            "#;
            
            let mut response = self.db.query_with_params(query, json!({
                "user_id": id
            })).await?;
            let results: Vec<Value> = response.take(0)?;
            
            if let Some(author) = results.first() {
                let display_name = author["display_name"].as_str().unwrap_or("").to_string();
                let username = author["username"].as_str().unwrap_or("").to_string();
                let avatar_url = author["avatar_url"].as_str().map(String::from);
                
                authors.insert(id.to_string(), (display_name, username, avatar_url));
            }
        }

        Ok(authors)
    }

    async fn get_user_comment_claps(
        &self,
        user_id: &str,
        comments: &[Comment],
    ) -> Result<HashMap<String, bool>> {
        let mut claps = HashMap::new();
        let comment_ids: Vec<&str> = comments.iter().map(|c| c.id.as_str()).collect();

        if !comment_ids.is_empty() {
            let query = r#"
                SELECT comment_id 
                FROM comment_clap 
                WHERE user_id = $user_id 
                AND comment_id IN $comment_ids
            "#;

            let mut response = self.db.query_with_params(query, json!({
                "user_id": user_id,
                "comment_ids": comment_ids
            })).await?;

            let results: Vec<Value> = response.take(0)?;
            
            for result in results {
                if let Some(comment_id) = result["comment_id"].as_str() {
                    claps.insert(comment_id.to_string(), true);
                }
            }
        }

        Ok(claps)
    }

    async fn update_article_comment_count(&self, article_id: &str) -> Result<()> {
        let query = r#"
            LET $count = (SELECT count() FROM comment WHERE article_id = $article_id AND is_deleted = false);
            UPDATE article SET comment_count = $count WHERE id = $article_id;
        "#;

        self.db.query_with_params(query, json!({
            "article_id": article_id
        })).await?;

        Ok(())
    }

    async fn update_comment_clap_count(&self, comment_id: &str) -> Result<()> {
        let query = r#"
            LET $count = (SELECT count() FROM comment_clap WHERE comment_id = $comment_id);
            UPDATE comment SET clap_count = $count WHERE id = $comment_id;
        "#;

        self.db.query_with_params(query, json!({
            "comment_id": comment_id
        })).await?;

        Ok(())
    }
}