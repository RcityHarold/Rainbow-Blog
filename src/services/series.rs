use crate::{
    error::{AppError, Result},
    models::{
        series::*,
        article::{Article, ArticleStatus},
        user::UserProfile,
    },
    services::Database,
    utils::slug,
};
use chrono::Utc;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone)]
pub struct SeriesService {
    db: Arc<Database>,
}

impl SeriesService {
    pub async fn new(db: Arc<Database>) -> Result<Self> {
        Ok(Self { db })
    }

    /// 创建系列
    pub async fn create_series(
        &self,
        author_id: &str,
        request: CreateSeriesRequest,
    ) -> Result<Series> {
        debug!("Creating series: {} for author: {}", request.title, author_id);

        request.validate().map_err(|e| AppError::ValidatorError(e))?;

        let slug = self.generate_unique_slug(&request.title).await?;

        let series = Series {
            id: Uuid::new_v4().to_string(),
            title: request.title,
            description: request.description,
            slug,
            author_id: author_id.to_string(),
            cover_image_url: request.cover_image_url,
            article_count: 0,
            is_completed: false,
            is_public: request.is_public.unwrap_or(true),
            view_count: 0,
            subscriber_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let created_series: Series = self.db.create("series", series).await?;

        info!("Created series: {} ({})", created_series.title, created_series.id);
        Ok(created_series)
    }

    /// 获取系列详情
    pub async fn get_series(
        &self,
        slug: &str,
        user_id: Option<&str>,
    ) -> Result<Option<SeriesResponse>> {
        debug!("Getting series: {}", slug);

        let series: Option<Series> = self.db.find_one("series", "slug", slug).await?;
        let series = match series {
            Some(s) => s,
            None => return Ok(None),
        };

        // 检查访问权限
        if !series.is_public && user_id != Some(&series.author_id) {
            return Ok(None); // 私有系列只有作者能看到
        }

        // 获取作者信息
        let author_info = self.get_author_info(&series.author_id).await?;

        // 获取系列中的文章
        let articles = self.get_series_articles(&series.id, user_id).await?;

        // 检查用户是否订阅
        let is_subscribed = if let Some(uid) = user_id {
            self.is_subscribed(&series.id, uid).await?
        } else {
            false
        };

        // 增加浏览次数
        if user_id != Some(&series.author_id) {
            self.increment_view_count(&series.id).await?;
        }

        let response = SeriesResponse {
            series,
            author_name: author_info.0,
            author_username: author_info.1,
            author_avatar: author_info.2,
            is_subscribed,
            articles,
        };

        Ok(Some(response))
    }

    /// 更新系列
    pub async fn update_series(
        &self,
        series_id: &str,
        author_id: &str,
        request: UpdateSeriesRequest,
    ) -> Result<Series> {
        debug!("Updating series: {} by author: {}", series_id, author_id);

        request.validate().map_err(|e| AppError::ValidatorError(e))?;

        let mut series: Series = self.db.get_by_id("series", series_id).await?
            .ok_or_else(|| AppError::NotFound("Series not found".to_string()))?;

        // 检查权限
        if series.author_id != author_id {
            return Err(AppError::forbidden("Only the author can update this series"));
        }

        // 更新字段
        if let Some(title) = request.title {
            if title != series.title {
                series.title = title;
                series.slug = self.generate_unique_slug(&series.title).await?;
            }
        }

        if let Some(description) = request.description {
            series.description = Some(description);
        }

        if let Some(cover_image_url) = request.cover_image_url {
            series.cover_image_url = Some(cover_image_url);
        }

        if let Some(is_public) = request.is_public {
            series.is_public = is_public;
        }

        if let Some(is_completed) = request.is_completed {
            series.is_completed = is_completed;
        }

        series.updated_at = Utc::now();

        let updated: Series = self.db.update_by_id("series", series_id, series).await?
            .ok_or_else(|| AppError::internal("Failed to update series"))?;

        Ok(updated)
    }

    /// 删除系列
    pub async fn delete_series(
        &self,
        series_id: &str,
        author_id: &str,
    ) -> Result<()> {
        debug!("Deleting series: {} by author: {}", series_id, author_id);

        let series: Series = self.db.get_by_id("series", series_id).await?
            .ok_or_else(|| AppError::NotFound("Series not found".to_string()))?;

        if series.author_id != author_id {
            return Err(AppError::forbidden("Only the author can delete this series"));
        }

        // 删除系列文章关联
        let query = "DELETE series_article WHERE series_id = $series_id";
        self.db.query_with_params(query, json!({ "series_id": series_id })).await?;

        // 删除订阅关系
        let query = "DELETE series_subscription WHERE series_id = $series_id";
        self.db.query_with_params(query, json!({ "series_id": series_id })).await?;

        // 删除系列
        self.db.delete_by_id("series", series_id).await?;

        info!("Deleted series: {}", series_id);
        Ok(())
    }

    /// 获取系列列表
    pub async fn get_series_list(
        &self,
        query: SeriesQuery,
    ) -> Result<crate::services::database::PaginatedResult<SeriesListItem>> {
        debug!("Getting series list with query: {:?}", query);

        let page = query.page.unwrap_or(1);
        let limit = query.limit.unwrap_or(20);
        let offset = (page - 1) * limit;

        let mut conditions = Vec::new();

        if let Some(author_id) = &query.author_id {
            conditions.push("s.author_id = $author_id".to_string());
        }

        if let Some(is_completed) = query.is_completed {
            conditions.push(format!("s.is_completed = {}", is_completed));
        }

        if let Some(is_public) = query.is_public {
            conditions.push(format!("s.is_public = {}", is_public));
        }

        if let Some(search) = &query.search {
            conditions.push("(s.title ~ $search OR s.description ~ $search)".to_string());
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let order_by = match query.sort.as_deref() {
            Some("oldest") => "s.created_at ASC",
            Some("popular") => "s.view_count DESC",
            Some("alphabetical") => "s.title ASC",
            _ => "s.created_at DESC", // newest
        };

        let count_query = format!(
            "SELECT count() AS total FROM series s {}",
            where_clause
        );

        let data_query = format!(
            r#"
            SELECT s.id, s.title, s.slug, s.description, s.cover_image_url,
                   s.author_id, s.article_count, s.is_completed, s.created_at,
                   u.display_name as author_name, u.username as author_username
            FROM series s
            JOIN user_profile u ON s.author_id = u.user_id
            {}
            ORDER BY {}
            LIMIT $limit START $offset
            "#,
            where_clause, order_by
        );

        let mut params = json!({
            "limit": limit,
            "offset": offset
        });

        if let Some(author_id) = &query.author_id {
            params["author_id"] = json!(author_id);
        }

        if let Some(search) = &query.search {
            params["search"] = json!(search);
        }

        let mut count_response = self.db.query_with_params(&count_query, &params).await?;
        let total = if let Ok(Some(result)) = count_response.take::<Option<Value>>(0) {
            result.get("total").and_then(|v| v.as_i64()).unwrap_or(0) as usize
        } else { 0 };

        let mut data_response = self.db.query_with_params(&data_query, params).await?;
        let series_list: Vec<SeriesListItem> = data_response.take(0)?;

        Ok(crate::services::database::PaginatedResult {
            data: series_list,
            total,
            page,
            per_page: limit,
            total_pages: (total + limit - 1) / limit,
        })
    }

    /// 添加文章到系列
    pub async fn add_article_to_series(
        &self,
        series_id: &str,
        author_id: &str,
        request: AddArticleToSeriesRequest,
    ) -> Result<SeriesArticle> {
        debug!("Adding article {} to series: {}", request.article_id, series_id);

        request.validate().map_err(|e| AppError::ValidatorError(e))?;

        // 验证系列存在且属于作者
        let series: Series = self.db.get_by_id("series", series_id).await?
            .ok_or_else(|| AppError::NotFound("Series not found".to_string()))?;

        if series.author_id != author_id {
            return Err(AppError::forbidden("Only the author can add articles to this series"));
        }

        // 验证文章存在且属于同一作者
        let article: Article = self.db.get_by_id("article", &request.article_id).await?
            .ok_or_else(|| AppError::NotFound("Article not found".to_string()))?;

        if article.author_id != author_id {
            return Err(AppError::forbidden("You can only add your own articles to the series"));
        }

        // 检查文章是否已在系列中
        let exists_query = r#"
            SELECT count() as count 
            FROM series_article 
            WHERE series_id = $series_id 
            AND article_id = $article_id
        "#;

        let mut response = self.db.query_with_params(exists_query, json!({
            "series_id": series_id,
            "article_id": &request.article_id
        })).await?;

        let exists: Vec<Value> = response.take(0)?;
        if let Some(result) = exists.first() {
            if result.get("count").and_then(|v| v.as_i64()).unwrap_or(0) > 0 {
                return Err(AppError::Conflict("Article is already in the series".to_string()));
            }
        }

        // 获取下一个order_index
        let order_index = if let Some(idx) = request.order_index {
            idx
        } else {
            self.get_next_order_index(series_id).await?
        };

        let series_article = SeriesArticle {
            id: Uuid::new_v4().to_string(),
            series_id: series_id.to_string(),
            article_id: request.article_id,
            order_index,
            added_at: Utc::now(),
        };

        let created: SeriesArticle = self.db.create("series_article", series_article).await?;

        // 更新系列的文章数量
        self.update_article_count(series_id).await?;

        // 更新文章的系列信息
        self.update_article_series_info(&created.article_id, series_id, order_index).await?;

        Ok(created)
    }

    /// 从系列中移除文章
    pub async fn remove_article_from_series(
        &self,
        series_id: &str,
        article_id: &str,
        author_id: &str,
    ) -> Result<()> {
        debug!("Removing article {} from series: {}", article_id, series_id);

        // 验证系列权限
        let series: Series = self.db.get_by_id("series", series_id).await?
            .ok_or_else(|| AppError::NotFound("Series not found".to_string()))?;

        if series.author_id != author_id {
            return Err(AppError::forbidden("Only the author can remove articles from this series"));
        }

        // 删除关联
        let query = r#"
            DELETE series_article 
            WHERE series_id = $series_id 
            AND article_id = $article_id
        "#;

        self.db.query_with_params(query, json!({
            "series_id": series_id,
            "article_id": article_id
        })).await?;

        // 更新系列的文章数量
        self.update_article_count(series_id).await?;

        // 清除文章的系列信息
        self.clear_article_series_info(article_id).await?;

        // 重新排序剩余文章
        self.reorder_series_articles(series_id).await?;

        Ok(())
    }

    /// 更新文章顺序
    pub async fn update_article_order(
        &self,
        series_id: &str,
        author_id: &str,
        request: UpdateArticleOrderRequest,
    ) -> Result<()> {
        debug!("Updating article order for series: {}", series_id);

        request.validate().map_err(|e| AppError::ValidatorError(e))?;

        // 验证系列权限
        let series: Series = self.db.get_by_id("series", series_id).await?
            .ok_or_else(|| AppError::NotFound("Series not found".to_string()))?;

        if series.author_id != author_id {
            return Err(AppError::forbidden("Only the author can reorder articles in this series"));
        }

        // 批量更新文章顺序
        for item in &request.articles {
            let query = r#"
                UPDATE series_article 
                SET order_index = $order_index 
                WHERE series_id = $series_id 
                AND article_id = $article_id
            "#;

            self.db.query_with_params(query, json!({
                "series_id": series_id,
                "article_id": &item.article_id,
                "order_index": item.order_index
            })).await?;

            // 同时更新文章表中的series_order
            self.update_article_series_info(&item.article_id, series_id, item.order_index).await?;
        }

        Ok(())
    }

    /// 订阅系列
    pub async fn subscribe_series(
        &self,
        series_id: &str,
        user_id: &str,
    ) -> Result<()> {
        debug!("User {} subscribing to series: {}", user_id, series_id);

        // 验证系列存在
        let _series: Series = self.db.get_by_id("series", series_id).await?
            .ok_or_else(|| AppError::NotFound("Series not found".to_string()))?;

        // 检查是否已订阅
        if self.is_subscribed(series_id, user_id).await? {
            return Err(AppError::Conflict("Already subscribed to this series".to_string()));
        }

        let subscription = SeriesSubscription {
            id: Uuid::new_v4().to_string(),
            series_id: series_id.to_string(),
            user_id: user_id.to_string(),
            created_at: Utc::now(),
        };

        self.db.create("series_subscription", subscription).await?;

        // 更新订阅者数量
        self.update_subscriber_count(series_id).await?;

        Ok(())
    }

    /// 取消订阅系列
    pub async fn unsubscribe_series(
        &self,
        series_id: &str,
        user_id: &str,
    ) -> Result<()> {
        debug!("User {} unsubscribing from series: {}", user_id, series_id);

        let query = r#"
            DELETE series_subscription 
            WHERE series_id = $series_id 
            AND user_id = $user_id
        "#;

        self.db.query_with_params(query, json!({
            "series_id": series_id,
            "user_id": user_id
        })).await?;

        // 更新订阅者数量
        self.update_subscriber_count(series_id).await?;

        Ok(())
    }

    /// 获取用户订阅的系列
    pub async fn get_user_subscribed_series(
        &self,
        user_id: &str,
        page: usize,
        limit: usize,
    ) -> Result<crate::services::database::PaginatedResult<SeriesListItem>> {
        debug!("Getting subscribed series for user: {}", user_id);

        let offset = (page - 1) * limit;

        let count_query = r#"
            SELECT count() AS total 
            FROM series_subscription 
            WHERE user_id = $user_id
        "#;

        let data_query = r#"
            SELECT s.id, s.title, s.slug, s.description, s.cover_image_url,
                   s.author_id, s.article_count, s.is_completed, s.created_at,
                   u.display_name as author_name, u.username as author_username
            FROM series s
            JOIN series_subscription sub ON s.id = sub.series_id
            JOIN user_profile u ON s.author_id = u.user_id
            WHERE sub.user_id = $user_id
            ORDER BY sub.created_at DESC
            LIMIT $limit START $offset
        "#;

        let params = json!({
            "user_id": user_id,
            "limit": limit,
            "offset": offset
        });

        let mut count_response = self.db.query_with_params(count_query, &params).await?;
        let total = if let Ok(Some(result)) = count_response.take::<Option<Value>>(0) {
            result.get("total").and_then(|v| v.as_i64()).unwrap_or(0) as usize
        } else { 0 };

        let mut data_response = self.db.query_with_params(data_query, params).await?;
        let series_list: Vec<SeriesListItem> = data_response.take(0)?;

        Ok(crate::services::database::PaginatedResult {
            data: series_list,
            total,
            page,
            per_page: limit,
            total_pages: (total + limit - 1) / limit,
        })
    }

    // Helper methods

    async fn generate_unique_slug(&self, title: &str) -> Result<String> {
        let base_slug = slug::generate_slug(title);
        let mut slug = base_slug.clone();
        let mut counter = 1;

        while self.db.find_one::<Value>("series", "slug", &slug).await?.is_some() {
            slug = format!("{}-{}", base_slug, counter);
            counter += 1;

            if counter > 100 {
                return Err(AppError::internal("Failed to generate unique slug"));
            }
        }

        Ok(slug)
    }

    async fn get_author_info(&self, author_id: &str) -> Result<(String, String, Option<String>)> {
        let query = r#"
            SELECT display_name, username, avatar_url 
            FROM user_profile 
            WHERE user_id = $author_id
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "author_id": author_id
        })).await?;

        let results: Vec<Value> = response.take(0)?;
        if let Some(author) = results.first() {
            let display_name = author["display_name"].as_str().unwrap_or("").to_string();
            let username = author["username"].as_str().unwrap_or("").to_string();
            let avatar_url = author["avatar_url"].as_str().map(String::from);
            Ok((display_name, username, avatar_url))
        } else {
            Err(AppError::NotFound("Author not found".to_string()))
        }
    }

    async fn get_series_articles(
        &self,
        series_id: &str,
        user_id: Option<&str>,
    ) -> Result<Vec<SeriesArticleInfo>> {
        let query = r#"
            SELECT a.id, a.title, a.subtitle, a.slug, a.excerpt, a.cover_image_url,
                   a.reading_time, a.status, a.published_at, sa.order_index
            FROM article a
            JOIN series_article sa ON a.id = sa.article_id
            WHERE sa.series_id = $series_id
            ORDER BY sa.order_index ASC, sa.added_at ASC
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "series_id": series_id
        })).await?;

        let results: Vec<Value> = response.take(0)?;
        let mut articles = Vec::new();

        for result in results {
            // 检查文章是否公开或者用户是作者
            let status = result["status"].as_str().unwrap_or("");
            let article_author_id = result["author_id"].as_str();
            
            let is_published = status == "published";
            let can_view = is_published || user_id == article_author_id;

            if can_view {
                articles.push(SeriesArticleInfo {
                    id: result["id"].as_str().unwrap_or("").to_string(),
                    title: result["title"].as_str().unwrap_or("").to_string(),
                    subtitle: result["subtitle"].as_str().map(String::from),
                    slug: result["slug"].as_str().unwrap_or("").to_string(),
                    excerpt: result["excerpt"].as_str().map(String::from),
                    cover_image_url: result["cover_image_url"].as_str().map(String::from),
                    reading_time: result["reading_time"].as_i64().unwrap_or(0) as i32,
                    order_index: result["order_index"].as_i64().unwrap_or(0) as i32,
                    is_published,
                    published_at: result["published_at"]
                        .as_str()
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                });
            }
        }

        Ok(articles)
    }

    async fn is_subscribed(&self, series_id: &str, user_id: &str) -> Result<bool> {
        let query = r#"
            SELECT count() as count 
            FROM series_subscription 
            WHERE series_id = $series_id 
            AND user_id = $user_id
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "series_id": series_id,
            "user_id": user_id
        })).await?;

        let result: Vec<Value> = response.take(0)?;
        let count = result.first()
            .and_then(|v| v.get("count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        Ok(count > 0)
    }

    async fn increment_view_count(&self, series_id: &str) -> Result<()> {
        let query = "UPDATE series SET view_count += 1 WHERE id = $series_id";
        self.db.query_with_params(query, json!({ "series_id": series_id })).await?;
        Ok(())
    }

    async fn update_article_count(&self, series_id: &str) -> Result<()> {
        let query = r#"
            LET $count = (SELECT count() FROM series_article WHERE series_id = $series_id);
            UPDATE series SET article_count = $count WHERE id = $series_id;
        "#;

        self.db.query_with_params(query, json!({ "series_id": series_id })).await?;
        Ok(())
    }

    async fn update_subscriber_count(&self, series_id: &str) -> Result<()> {
        let query = r#"
            LET $count = (SELECT count() FROM series_subscription WHERE series_id = $series_id);
            UPDATE series SET subscriber_count = $count WHERE id = $series_id;
        "#;

        self.db.query_with_params(query, json!({ "series_id": series_id })).await?;
        Ok(())
    }

    async fn get_next_order_index(&self, series_id: &str) -> Result<i32> {
        let query = r#"
            SELECT MAX(order_index) as max_index 
            FROM series_article 
            WHERE series_id = $series_id
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "series_id": series_id
        })).await?;

        let result: Vec<Value> = response.take(0)?;
        let max_index = result.first()
            .and_then(|v| v.get("max_index"))
            .and_then(|v| v.as_i64())
            .unwrap_or(-1) as i32;

        Ok(max_index + 1)
    }

    async fn update_article_series_info(
        &self,
        article_id: &str,
        series_id: &str,
        order_index: i32,
    ) -> Result<()> {
        let updates = json!({
            "series_id": series_id,
            "series_order": order_index,
            "updated_at": Utc::now()
        });

        self.db.update_by_id_with_json::<Value>("article", article_id, updates).await?;
        Ok(())
    }

    async fn clear_article_series_info(&self, article_id: &str) -> Result<()> {
        let updates = json!({
            "series_id": null,
            "series_order": null,
            "updated_at": Utc::now()
        });

        self.db.update_by_id_with_json::<Value>("article", article_id, updates).await?;
        Ok(())
    }

    async fn reorder_series_articles(&self, series_id: &str) -> Result<()> {
        let query = r#"
            SELECT article_id, order_index 
            FROM series_article 
            WHERE series_id = $series_id 
            ORDER BY order_index ASC
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "series_id": series_id
        })).await?;

        let articles: Vec<Value> = response.take(0)?;

        // 重新分配order_index，确保连续
        for (index, article) in articles.iter().enumerate() {
            if let Some(article_id) = article["article_id"].as_str() {
                let new_index = index as i32;
                let update_query = r#"
                    UPDATE series_article 
                    SET order_index = $order_index 
                    WHERE series_id = $series_id 
                    AND article_id = $article_id
                "#;

                self.db.query_with_params(update_query, json!({
                    "series_id": series_id,
                    "article_id": article_id,
                    "order_index": new_index
                })).await?;

                self.update_article_series_info(article_id, series_id, new_index).await?;
            }
        }

        Ok(())
    }
}