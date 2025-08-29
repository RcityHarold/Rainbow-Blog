use crate::{
    error::{AppError, Result},
    models::tag::*,
    services::Database,
    utils::slug,
};
use chrono::Utc;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, info};
use validator::Validate;
use uuid::Uuid;

#[derive(Clone)]
pub struct TagService {
    db: Arc<Database>,
}

impl TagService {
    pub async fn new(db: Arc<Database>) -> Result<Self> {
        Ok(Self { db })
    }

    pub async fn create_tag(&self, request: CreateTagRequest) -> Result<Tag> {
        debug!("Creating tag: {}", request.name);

        request
            .validate()
            .map_err(|e| AppError::ValidatorError(e))?;

        // Check if tag name already exists
        let mut response = self.db.query_with_params(
            r#"
                SELECT * FROM tag 
                WHERE name = $name
            "#,
            json!({
                "name": &request.name
            })
        ).await?;
        let existing: Vec<Tag> = response.take(0)?;

        if !existing.is_empty() {
            return Err(AppError::Conflict(
                format!("Tag '{}' already exists", request.name),
            ));
        }

        let tag = Tag {
            id: Uuid::new_v4().to_string(),
            name: request.name.clone(),
            slug: slug::generate_slug(&request.name),
            description: request.description,
            follower_count: 0,
            article_count: 0,
            is_featured: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let created: Tag = self.db.create("tag", tag).await?;
        
        info!("Created tag: {} ({})", created.name, created.id);
        Ok(created)
    }

    pub async fn get_tags(&self, query: TagQuery) -> Result<Vec<Tag>> {
        debug!("Getting tags with query: {:?}", query);

        let page = query.page.unwrap_or(1).max(1);
        let limit = query.limit.unwrap_or(20).min(100);
        let offset = (page - 1) * limit;

        let mut sql = String::from("SELECT * FROM tag WHERE 1=1");
        let mut bindings = vec![];

        // Add search condition
        if let Some(search) = &query.search {
            sql.push_str(" AND (name CONTAINS $search OR description CONTAINS $search)");
            bindings.push(("search", json!(search)));
        }

        // Add featured filter
        if query.featured_only.unwrap_or(false) {
            sql.push_str(" AND is_featured = true");
        }

        // Add sorting
        match query.sort_by.as_deref() {
            Some("popular") => sql.push_str(" ORDER BY article_count DESC"),
            Some("name") => sql.push_str(" ORDER BY name ASC"),
            Some("created_at") => sql.push_str(" ORDER BY created_at DESC"),
            _ => sql.push_str(" ORDER BY article_count DESC"), // Default to popular
        }

        // Add pagination
        sql.push_str(" LIMIT $limit START $offset");
        bindings.push(("limit", json!(limit)));
        bindings.push(("offset", json!(offset)));

        let mut params = serde_json::Map::new();
        for (key, value) in bindings {
            params.insert(key.to_string(), value);
        }
        
        let mut response = self.db.query_with_params(&sql, json!(params)).await?;
        let tags: Vec<Tag> = response.take(0)?;

        Ok(tags)
    }

    pub async fn get_tag_by_id(&self, tag_id: &str) -> Result<Option<Tag>> {
        let tag: Option<Tag> = self.db.get_by_id("tag", tag_id).await?;
        Ok(tag)
    }

    pub async fn get_tag_by_slug(&self, slug: &str) -> Result<Option<Tag>> {
        let mut response = self.db.query_with_params(
            "SELECT * FROM tag WHERE slug = $slug",
            json!({ "slug": slug })
        ).await?;
        let tags: Vec<Tag> = response.take(0)?;

        Ok(tags.into_iter().next())
    }

    pub async fn update_tag(&self, tag_id: &str, request: UpdateTagRequest) -> Result<Tag> {
        debug!("Updating tag: {}", tag_id);

        request
            .validate()
            .map_err(|e| AppError::ValidatorError(e))?;

        let tag: Tag = self
            .db
            .get_by_id("tag", tag_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Tag not found".to_string()))?;

        // Check if new name conflicts with existing tag
        if let Some(ref new_name) = request.name {
            if new_name != &tag.name {
                let mut response = self.db.query_with_params(
                    "SELECT * FROM tag WHERE name = $name AND id != $id",
                    json!({
                        "name": new_name,
                        "id": tag_id
                    })
                ).await?;
                let existing: Vec<Tag> = response.take(0)?;

                if !existing.is_empty() {
                    return Err(AppError::Conflict(
                        format!("Tag '{}' already exists", new_name),
                    ));
                }
            }
        }

        let mut updates = json!({
            "updated_at": Utc::now(),
        });

        if let Some(name) = request.name {
            updates["name"] = json!(name.clone());
            updates["slug"] = json!(slug::generate_slug(&name));
        }
        
        if let Some(description) = request.description {
            updates["description"] = json!(description);
        }
        
        if let Some(is_featured) = request.is_featured {
            updates["is_featured"] = json!(is_featured);
        }

        let updated: Tag = self.db.update_by_id_with_json("tag", tag_id, updates).await?.ok_or_else(|| AppError::internal("Failed to update tag"))?;
        
        Ok(updated)
    }

    pub async fn delete_tag(&self, tag_id: &str) -> Result<()> {
        debug!("Deleting tag: {}", tag_id);

        let tag: Tag = self
            .db
            .get_by_id("tag", tag_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Tag not found".to_string()))?;

        // Check if tag is used by any articles
        if tag.article_count > 0 {
            return Err(AppError::Conflict(
                format!("Cannot delete tag '{}' as it is used by {} articles", tag.name, tag.article_count),
            ));
        }

        // Delete all user follows for this tag
        self.db.query_with_params(
            "DELETE user_tag_follow WHERE tag_id = $tag_id",
            json!({ "tag_id": tag_id })
        ).await?;

        // Delete the tag
        self.db.delete_by_id("tag", tag_id).await?;
        
        info!("Deleted tag: {} ({})", tag.name, tag_id);
        Ok(())
    }

    pub async fn add_tags_to_article(&self, article_id: &str, tag_ids: Vec<String>) -> Result<()> {
        debug!("Adding {} tags to article: {}", tag_ids.len(), article_id);

        for tag_id in tag_ids {
            // Check if tag exists
            let tag: Option<Tag> = self.db.get_by_id("tag", &tag_id).await?;
            if tag.is_none() {
                return Err(AppError::NotFound(format!("Tag {} not found", tag_id)));
            }

            // Check if association already exists
            let mut response = self.db.query_with_params(
                r#"
                    SELECT * FROM article_tag 
                    WHERE article_id = $article_id 
                    AND tag_id = $tag_id
                "#,
                json!({
                    "article_id": article_id,
                    "tag_id": &tag_id
                })
            ).await?;
            let existing: Vec<ArticleTag> = response.take(0)?;

            if existing.is_empty() {
                let article_tag = ArticleTag {
                    id: Uuid::new_v4().to_string(),
                    article_id: article_id.to_string(),
                    tag_id: tag_id.clone(),
                    created_at: Utc::now(),
                };

                self.db.create("article_tag", article_tag).await?;

                // Update tag article count
                self.update_tag_article_count(&tag_id).await?;
            }
        }

        Ok(())
    }

    pub async fn remove_tags_from_article(
        &self,
        article_id: &str,
        tag_ids: Vec<String>,
    ) -> Result<()> {
        debug!("Removing {} tags from article: {}", tag_ids.len(), article_id);

        for tag_id in tag_ids {
            self.db.query_with_params(
                r#"
                    DELETE article_tag 
                    WHERE article_id = $article_id 
                    AND tag_id = $tag_id
                "#,
                json!({
                    "article_id": article_id,
                    "tag_id": &tag_id
                })
            ).await?;

            // Update tag article count
            self.update_tag_article_count(&tag_id).await?;
        }

        Ok(())
    }

    pub async fn get_article_tags(&self, article_id: &str) -> Result<Vec<Tag>> {
        let query = r#"
            SELECT t.* FROM tag t
            JOIN article_tag at ON t.id = at.tag_id
            WHERE at.article_id = $article_id
            ORDER BY t.name
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "article_id": article_id
        })).await?;
        let tags: Vec<Tag> = response.take(0)?;

        Ok(tags)
    }

    pub async fn follow_tag(&self, tag_id: &str, user_id: &str) -> Result<()> {
        debug!("User {} following tag: {}", user_id, tag_id);

        // Check if tag exists
        let tag: Option<Tag> = self.db.get_by_id("tag", tag_id).await?;
        if tag.is_none() {
            return Err(AppError::NotFound("Tag not found".to_string()));
        }

        // Check if already following
        let mut response = self.db.query_with_params(
            r#"
                SELECT * FROM user_tag_follow 
                WHERE user_id = $user_id 
                AND tag_id = $tag_id
            "#,
            json!({
                "user_id": user_id,
                "tag_id": tag_id
            })
        ).await?;
        let existing: Vec<UserTagFollow> = response.take(0)?;

        if !existing.is_empty() {
            return Err(AppError::Conflict("Already following this tag".to_string()));
        }

        let follow = UserTagFollow {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            tag_id: tag_id.to_string(),
            created_at: Utc::now(),
        };

        self.db.create("user_tag_follow", follow).await?;

        // Update tag follower count
        self.update_tag_follower_count(tag_id).await?;

        Ok(())
    }

    pub async fn unfollow_tag(&self, tag_id: &str, user_id: &str) -> Result<()> {
        debug!("User {} unfollowing tag: {}", user_id, tag_id);

        self.db.query_with_params(
            r#"
                DELETE user_tag_follow 
                WHERE user_id = $user_id 
                AND tag_id = $tag_id
            "#,
            json!({
                "user_id": user_id,
                "tag_id": tag_id
            })
        ).await?;

        // Update tag follower count
        self.update_tag_follower_count(tag_id).await?;

        Ok(())
    }

    pub async fn get_user_followed_tags(&self, user_id: &str) -> Result<Vec<Tag>> {
        let query = r#"
            SELECT t.* FROM tag t
            JOIN user_tag_follow utf ON t.id = utf.tag_id
            WHERE utf.user_id = $user_id
            ORDER BY utf.created_at DESC
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "user_id": user_id
        })).await?;
        let tags: Vec<Tag> = response.take(0)?;

        Ok(tags)
    }

    pub async fn get_tags_with_follow_status(
        &self,
        user_id: Option<&str>,
        tag_ids: Vec<String>,
    ) -> Result<Vec<TagWithFollowStatus>> {
        let mut response = self.db.query_with_params(
            "SELECT * FROM tag WHERE id IN $tag_ids",
            json!({ "tag_ids": tag_ids.clone() })
        ).await?;
        let tags: Vec<Tag> = response.take(0)?;

        let mut result = Vec::new();

        if let Some(uid) = user_id {
            let mut response = self.db.query_with_params(
                r#"
                    SELECT * FROM user_tag_follow 
                    WHERE user_id = $user_id 
                    AND tag_id IN $tag_ids
                "#,
                json!({
                    "user_id": uid,
                    "tag_ids": tag_ids
                })
            ).await?;
            let followed: Vec<UserTagFollow> = response.take(0)?;

            let followed_set: std::collections::HashSet<String> =
                followed.into_iter().map(|f| f.tag_id).collect();

            for tag in tags {
                result.push(TagWithFollowStatus {
                    is_following: followed_set.contains(&tag.id),
                    tag,
                });
            }
        } else {
            for tag in tags {
                result.push(TagWithFollowStatus {
                    tag,
                    is_following: false,
                });
            }
        }

        Ok(result)
    }

    async fn update_tag_article_count(&self, tag_id: &str) -> Result<()> {
        let query = r#"
            LET $count = (SELECT count() FROM article_tag WHERE tag_id = $tag_id);
            UPDATE tag SET article_count = $count WHERE id = $tag_id;
        "#;

        self.db.query_with_params(query, json!({
            "tag_id": tag_id
        })).await?;

        Ok(())
    }

    async fn update_tag_follower_count(&self, tag_id: &str) -> Result<()> {
        let query = r#"
            LET $count = (SELECT count() FROM user_tag_follow WHERE tag_id = $tag_id);
            UPDATE tag SET follower_count = $count WHERE id = $tag_id;
        "#;

        self.db.query_with_params(query, json!({
            "tag_id": tag_id
        })).await?;

        Ok(())
    }
}