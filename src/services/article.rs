use crate::{
    error::{AppError, Result},
    models::article::*,
    services::Database,
    utils::{markdown::MarkdownProcessor, slug},
};
use chrono::Utc;
use serde_json::{json, Value};
use tracing::{info, warn, error, debug};
use validator::Validate;
use std::collections::HashMap;
use std::sync::Arc;
use soulcore::prelude::Thing;
use uuid::Uuid;

#[derive(Clone)]
pub struct ArticleService {
    db: Arc<Database>,
    markdown_processor: MarkdownProcessor,
}

impl ArticleService {
    pub async fn new(db: Arc<Database>) -> Result<Self> {
        let markdown_processor = MarkdownProcessor::new();

        Ok(Self {
            db,
            markdown_processor,
        })
    }

    /// 创建新文章
    pub async fn create_article(&self, author_id: &str, request: CreateArticleRequest) -> Result<Article> {
        debug!("Creating article for user: {}", author_id);

        // 验证输入
        request.validate()
            .map_err(|e| AppError::ValidatorError(e))?;

        // 创建文章对象
        let mut article = Article {
            id: Uuid::new_v4().to_string(),
            title: request.title,
            subtitle: request.subtitle,
            slug: String::new(), // 稍后生成
            content: request.content,
            content_html: String::new(), // 稍后生成
            excerpt: request.excerpt,
            cover_image_url: request.cover_image_url,
            author_id: author_id.to_string(),
            publication_id: request.publication_id,
            series_id: request.series_id,
            series_order: request.series_order,
            status: if request.save_as_draft.unwrap_or(true) { ArticleStatus::Draft } else { ArticleStatus::Published },
            is_paid_content: request.is_paid_content.unwrap_or(false),
            is_featured: false,
            reading_time: 0, // 稍后计算
            word_count: 0, // 稍后计算
            view_count: 0,
            clap_count: 0,
            comment_count: 0,
            bookmark_count: 0,
            share_count: 0,
            seo_title: request.seo_title,
            seo_description: request.seo_description,
            seo_keywords: request.seo_keywords.unwrap_or_default(),
            metadata: serde_json::json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            published_at: None,
            last_edited_at: None,
            is_deleted: false,
            deleted_at: None,
        };

        // 生成唯一的 slug
        article.slug = self.generate_unique_slug(&article.title).await?;

        // 处理 Markdown 内容
        article.content_html = self.markdown_processor.to_html(&article.content);
        
        // 计算阅读时间和字数
        article.reading_time = self.markdown_processor.estimate_reading_time(&article.content);
        article.word_count = self.markdown_processor.count_words(&article.content) as i32;
        
        // 如果没有提供摘要，自动生成
        if article.excerpt.is_none() {
            article.excerpt = Some(self.markdown_processor.generate_excerpt(&article.content, 300));
        }

        // 如果没有封面图，尝试从内容中提取
        if article.cover_image_url.is_none() {
            article.cover_image_url = self.markdown_processor.extract_cover_image(&article.content);
        }

        // 如果是发布状态，设置发布时间
        if article.status == ArticleStatus::Published {
            article.published_at = Some(Utc::now());
        }

        // 构建动态字段列表
        let mut fields = vec![
            "title: $title".to_string(),
            "slug: $slug".to_string(),
            "content: $content".to_string(),
            "content_html: $content_html".to_string(),
            "author_id: $author_id".to_string(),
            "status: $status".to_string(),
            "is_paid_content: $is_paid_content".to_string(),
            "is_featured: $is_featured".to_string(),
            "reading_time: $reading_time".to_string(),
            "word_count: $word_count".to_string(),
            "view_count: 0".to_string(),
            "clap_count: 0".to_string(),
            "comment_count: 0".to_string(),
            "bookmark_count: 0".to_string(),
            "share_count: 0".to_string(),
            "seo_keywords: $seo_keywords".to_string(),
            "metadata: $metadata".to_string(),
            "created_at: time::now()".to_string(),
            "updated_at: time::now()".to_string(),
            "is_deleted: false".to_string(),
        ];

        // 只添加有值的可选字段
        if article.subtitle.is_some() {
            fields.push("subtitle: $subtitle".to_string());
        }
        if article.excerpt.is_some() {
            fields.push("excerpt: $excerpt".to_string());
        }
        if article.cover_image_url.is_some() {
            fields.push("cover_image_url: $cover_image_url".to_string());
        }
        if article.publication_id.is_some() {
            fields.push("publication_id: $publication_id".to_string());
        }
        if article.series_id.is_some() {
            fields.push("series_id: $series_id".to_string());
        }
        if article.series_order.is_some() {
            fields.push("series_order: $series_order".to_string());
        }
        if article.seo_title.is_some() {
            fields.push("seo_title: $seo_title".to_string());
        }
        if article.seo_description.is_some() {
            fields.push("seo_description: $seo_description".to_string());
        }
        if article.status == ArticleStatus::Published {
            fields.push("published_at: time::now()".to_string());
        }

        // 使用具体的记录 ID 创建
        let query = format!(
            "CREATE article:`{}` CONTENT {{ {} }} RETURN *",
            article.id,
            fields.join(", ")
        );
        
        let params = json!({
            "title": article.title,
            "subtitle": article.subtitle,
            "slug": article.slug,
            "content": article.content,
            "content_html": article.content_html,
            "excerpt": article.excerpt,
            "cover_image_url": article.cover_image_url,
            "author_id": article.author_id,
            "publication_id": article.publication_id,
            "series_id": article.series_id,
            "series_order": article.series_order,
            "status": serde_json::to_value(&article.status)?,
            "is_paid_content": article.is_paid_content,
            "is_featured": article.is_featured,
            "reading_time": article.reading_time,
            "word_count": article.word_count,
            "seo_title": article.seo_title,
            "seo_description": article.seo_description,
            "seo_keywords": article.seo_keywords,
            "metadata": article.metadata
        });
        
        let mut response = self.db.query_with_params(&query, params).await?;
        let created_articles: Vec<Article> = response.take(0)?;
        let created_article = created_articles.into_iter().next()
            .ok_or_else(|| AppError::Internal("Failed to create article".to_string()))?;

        // 处理标签（如果有）
        if let Some(tags) = &request.tags {
            self.attach_tags_to_article(&created_article.id, tags).await?;
        }

        info!("Created article: {} by user: {}", created_article.id, author_id);
        Ok(created_article)
    }

    /// 更新文章
    pub async fn update_article(&self, article_id: &str, author_id: &str, request: UpdateArticleRequest) -> Result<Article> {
        debug!("Updating article: {} by user: {}", article_id, author_id);

        // 验证输入
        request.validate()
            .map_err(|e| AppError::ValidatorError(e))?;

        // 获取现有文章
        let mut article = self.get_article_by_id(article_id).await?
            .ok_or_else(|| AppError::NotFound("Article not found".to_string()))?;

        // 检查权限
        if article.author_id != author_id {
            return Err(AppError::Authorization("Only article author can update this article".to_string()));
        }

        // 更新字段
        let mut content_updated = false;
        
        if let Some(title) = request.title {
            if title != article.title {
                article.title = title;
                // 生成新的 slug
                article.slug = self.generate_unique_slug(&article.title).await?;
            }
        }

        if let Some(content) = request.content {
            article.content = content;
            article.content_html = self.markdown_processor.to_html(&article.content);
            article.reading_time = self.markdown_processor.estimate_reading_time(&article.content);
            article.word_count = self.markdown_processor.count_words(&article.content) as i32;
            content_updated = true;
        }
        
        if let Some(subtitle) = request.subtitle {
            article.subtitle = Some(subtitle);
        }

        if let Some(excerpt) = request.excerpt {
            article.excerpt = Some(excerpt);
        }

        if let Some(cover_image_url) = request.cover_image_url {
            article.cover_image_url = Some(cover_image_url);
        }

        if let Some(publication_id) = request.publication_id {
            article.publication_id = Some(publication_id);
        }

        if let Some(series_id) = request.series_id {
            article.series_id = Some(series_id);
        }
        
        if let Some(series_order) = request.series_order {
            article.series_order = Some(series_order);
        }

        if let Some(status) = request.status {
            if article.status != ArticleStatus::Published && status == ArticleStatus::Published {
                // 首次发布
                article.published_at = Some(Utc::now());
            }
            article.status = status;
        }

        if let Some(is_paid_content) = request.is_paid_content {
            article.is_paid_content = is_paid_content;
        }
        
        if let Some(seo_title) = request.seo_title {
            article.seo_title = Some(seo_title);
        }
        
        if let Some(seo_description) = request.seo_description {
            article.seo_description = Some(seo_description);
        }
        
        if let Some(seo_keywords) = request.seo_keywords {
            article.seo_keywords = seo_keywords;
        }

        if let Some(metadata) = request.metadata {
            article.metadata = metadata;
        }

        // 更新时间戳
        article.updated_at = Utc::now();
        if content_updated {
            article.last_edited_at = Some(Utc::now());
        }

        // 更新文章
        let thing = Thing {
            tb: "article".to_string(),
            id: surrealdb::sql::Id::String(article_id.to_string()),
        };
        let updated_article = self.db.update(thing, article).await?
            .ok_or_else(|| AppError::NotFound("Failed to update article".to_string()))?;

        // 更新标签（如果提供）
        if let Some(tags) = request.tags {
            self.update_article_tags(&updated_article.id, &tags).await?;
        }

        info!("Updated article: {}", article_id);
        Ok(updated_article)
    }

    /// 软删除文章
    pub async fn delete_article(&self, article_id: &str, author_id: &str) -> Result<()> {
        debug!("Deleting article: {} by user: {}", article_id, author_id);

        // 获取文章以验证权限
        let article = self.get_article_by_id(article_id).await?
            .ok_or_else(|| AppError::NotFound("Article not found".to_string()))?;

        if article.author_id != author_id {
            return Err(AppError::Authorization("Only article author can delete this article".to_string()));
        }

        // 软删除
        let query = "UPDATE article SET is_deleted = true, updated_at = $now WHERE id = $id";
        self.db.query_with_params(query, json!({
            "id": article_id,
            "now": Utc::now()
        })).await?;

        info!("Deleted article: {}", article_id);
        Ok(())
    }

    /// 根据 ID 获取文章
    pub async fn get_article_by_id(&self, article_id: &str) -> Result<Option<Article>> {
        debug!("Getting article by ID: {}", article_id);

        // 获取纯 ID（不带 table 前缀）
        let pure_id = if article_id.starts_with("article:") {
            &article_id[8..]
        } else {
            article_id
        };

        // 使用反引号包裹 ID 以避免解析问题
        let query = format!("SELECT * FROM article:`{}`", pure_id);
        debug!("Executing query: {}", query);
        
        let mut response = self.db.query(&query).await?;
        
        let articles: Vec<Article> = response.take(0)?;
        debug!("Found {} articles", articles.len());
        Ok(articles.into_iter().next())
    }

    /// 根据 slug 获取文章
    pub async fn get_article_by_slug(&self, slug: &str) -> Result<Option<Article>> {
        debug!("Getting article by slug: {}", slug);

        self.db.find_one("article", "slug", slug).await
    }

    /// 获取文章完整信息（包含作者、标签、统计等）
    pub async fn get_article_with_details(&self, slug: &str, viewer_user_id: Option<&str>) -> Result<Option<ArticleResponse>> {
        debug!("Getting article with details for slug: {}", slug);

        // 获取文章基础信息
        let article = match self.get_article_by_slug(slug).await? {
            Some(article) => article,
            None => return Ok(None),
        };

        // 获取作者信息
        let author = self.get_article_author(&article.author_id).await?;

        // 获取文章标签
        let tags = self.get_article_tags(&article.id).await?;

        // 获取出版物信息（如果有）
        let publication = match &article.publication_id {
            Some(pub_id) => self.get_article_publication(pub_id).await?,
            None => None,
        };

        // 获取系列信息（如果有）
        let series = match &article.series_id {
            Some(series_id) => self.get_article_series(series_id, &article.id).await?,
            None => None,
        };

        // 获取用户相关信息（如果已登录）
        let (is_bookmarked, is_clapped, user_clap_count) = if let Some(user_id) = viewer_user_id {
            let bookmarked = self.is_article_bookmarked(&article.id, user_id).await?;
            let clapped = self.is_article_clapped(&article.id, user_id).await?;
            let clap_count = self.get_user_clap_count(&article.id, user_id).await?;
            (Some(bookmarked), Some(clapped), Some(clap_count))
        } else {
            (None, None, None)
        };

        let article_response = ArticleResponse {
            id: article.id,
            title: article.title,
            subtitle: article.subtitle,
            slug: article.slug,
            content: article.content,
            content_html: article.content_html,
            excerpt: article.excerpt,
            cover_image_url: article.cover_image_url,
            author,
            publication,
            series,
            status: article.status,
            is_paid_content: article.is_paid_content,
            is_featured: article.is_featured,
            reading_time: article.reading_time,
            word_count: article.word_count,
            view_count: article.view_count,
            clap_count: article.clap_count,
            comment_count: article.comment_count,
            bookmark_count: article.bookmark_count,
            share_count: article.share_count,
            tags,
            seo_title: article.seo_title,
            seo_description: article.seo_description,
            seo_keywords: article.seo_keywords,
            created_at: article.created_at,
            updated_at: article.updated_at,
            published_at: article.published_at,
            is_bookmarked,
            is_clapped,
            user_clap_count,
        };

        Ok(Some(article_response))
    }

    /// 获取文章列表（分页）
    pub async fn get_articles(&self, query: ArticleQuery) -> Result<crate::services::database::PaginatedResult<ArticleListItem>> {
        debug!("Getting articles list with query: {:?}", query);

        let page = query.page.unwrap_or(1);
        let limit = query.limit.unwrap_or(20);
        let offset = (page - 1) * limit;

        // 构建查询条件
        let mut conditions = vec!["is_deleted = false".to_string()];

        // 状态过滤
        if let Some(status) = &query.status {
            conditions.push(format!("status = '{}'", status));
        } else {
            conditions.push("status = 'published'".to_string());
        }

        // 作者过滤
        if let Some(author) = &query.author {
            conditions.push(format!("author_id = $author"));
        }

        // 标签过滤
        if let Some(tag) = &query.tag {
            conditions.push(format!("$tag IN tags"));
        }

        // 出版物过滤
        if let Some(publication) = &query.publication {
            conditions.push(format!("publication_id = $publication"));
        }

        // 精选文章过滤
        if let Some(featured) = query.featured {
            conditions.push(format!("is_featured = {}", featured));
        }

        // 搜索
        if let Some(search_term) = &query.search {
            conditions.push(format!("(title ~ $search OR content ~ $search)"));
        }

        let where_clause = conditions.join(" AND ");

        // 排序
        let (select_fields, order_by) = match query.sort.as_deref() {
            Some("oldest") => ("*", "created_at ASC"),
            Some("popular") => ("*", "clap_count DESC, view_count DESC"),
            Some("trending") => {
                // 在 SELECT 中计算趋势分数
                ("*, (clap_count + comment_count * 2 + view_count * 0.1) as trending_score", "trending_score DESC")
            },
            _ => ("*", "created_at DESC"),
        };

        // 构建查询
        let count_query = format!("SELECT count() AS total FROM article WHERE {}", where_clause);
        let data_query = format!(
            "SELECT {} FROM article WHERE {} ORDER BY {} LIMIT $limit START $offset",
            select_fields, where_clause, order_by
        );

        // 构建参数
        let mut params = json!({
            "limit": limit,
            "offset": offset
        });

        if let Some(author) = &query.author {
            params["author"] = json!(author);
        }
        if let Some(tag) = &query.tag {
            params["tag"] = json!(tag);
        }
        if let Some(publication) = &query.publication {
            params["publication"] = json!(publication);
        }
        if let Some(search_term) = &query.search {
            params["search"] = json!(search_term);
        }

        // 执行查询
        let mut count_response = self.db.query_with_params(&count_query, &params).await?;
        let total = if let Ok(Some(result)) = count_response.take::<Option<Value>>(0) {
            result.get("total").and_then(|v| v.as_i64()).unwrap_or(0) as usize
        } else { 0 };

        let mut data_response = self.db.query_with_params(&data_query, params).await?;
        let articles: Vec<Article> = data_response.take(0)?;
        
        // 将Article转换为ArticleListItem，并填充作者信息
        let mut article_list_items = Vec::new();
        for article in articles {
            let list_item = self.article_to_list_item(&article).await?;
            article_list_items.push(list_item);
        }

        Ok(crate::services::database::PaginatedResult {
            data: article_list_items,
            total,
            page,
            per_page: limit,
            total_pages: (total + limit - 1) / limit,
        })
    }

    /// 获取用户的文章列表
    pub async fn get_user_articles(&self, user_id: &str, page: usize, limit: usize, include_drafts: bool) -> Result<crate::services::database::PaginatedResult<ArticleListItem>> {
        debug!("Getting articles for user: {} (include_drafts: {})", user_id, include_drafts);

        let mut query = ArticleQuery {
            author: Some(user_id.to_string()),
            page: Some(page),
            limit: Some(limit),
            ..Default::default()
        };

        if include_drafts {
            query.status = None; // 返回所有状态的文章
        }

        self.get_articles(query).await
    }

    /// 增加文章浏览次数
    pub async fn increment_view_count(&self, article_id: &str) -> Result<()> {
        debug!("Incrementing view count for article: {}", article_id);

        let query = "UPDATE article SET view_count += 1, updated_at = $now WHERE id = $id";
        self.db.query_with_params(query, json!({
            "id": article_id,
            "now": Utc::now()
        })).await?;

        Ok(())
    }

    /// 增加文章鼓掌数
    pub async fn increment_clap_count(&self, article_id: &str, count: u32) -> Result<()> {
        debug!("Incrementing clap count for article: {} by {}", article_id, count);

        let query = "UPDATE article SET clap_count += $count, updated_at = $now WHERE id = $id";
        self.db.query_with_params(query, json!({
            "id": article_id,
            "count": count,
            "now": Utc::now()
        })).await?;

        Ok(())
    }

    /// 更新文章评论数
    pub async fn update_comment_count(&self, article_id: &str) -> Result<()> {
        debug!("Updating comment count for article: {}", article_id);

        let query = r#"
            LET $count = (SELECT count() FROM comment WHERE article_id = $id AND is_deleted = false);
            UPDATE article SET comment_count = $count, updated_at = $now WHERE id = $id;
        "#;
        
        self.db.query_with_params(query, json!({
            "id": article_id,
            "now": Utc::now()
        })).await?;

        Ok(())
    }

    /// 生成唯一的 slug
    async fn generate_unique_slug(&self, title: &str) -> Result<String> {
        let base_slug = slug::generate_slug(title);
        let mut slug = base_slug.clone();
        let mut counter = 1;

        while self.get_article_by_slug(&slug).await?.is_some() {
            slug = format!("{}-{}", base_slug, counter);
            counter += 1;
            
            if counter > 100 {
                return Err(AppError::Internal("Failed to generate unique slug".to_string()));
            }
        }

        Ok(slug)
    }

    /// 为文章附加标签
    async fn attach_tags_to_article(&self, article_id: &str, tags: &[String]) -> Result<()> {
        debug!("Attaching {} tags to article: {}", tags.len(), article_id);

        // 清理现有标签
        let clear_query = "DELETE article_tag WHERE article_id = $article_id";
        self.db.query_with_params(clear_query, json!({ "article_id": article_id })).await?;

        // 添加新标签
        for tag_name in tags {
            // 获取或创建标签
            let tag_id = self.get_or_create_tag(tag_name).await?;
            
            // 创建关联
            let create_query = "CREATE article_tag SET article_id = $article_id, tag_id = $tag_id";
            self.db.query_with_params(create_query, json!({
                "article_id": article_id,
                "tag_id": tag_id
            })).await?;
        }

        // 更新文章的标签字段
        let update_query = "UPDATE article SET tags = $tags WHERE id = $id";
        self.db.query_with_params(update_query, json!({
            "id": article_id,
            "tags": tags
        })).await?;

        Ok(())
    }

    /// 更新文章标签
    async fn update_article_tags(&self, article_id: &str, tags: &[String]) -> Result<()> {
        self.attach_tags_to_article(article_id, tags).await
    }

    /// 获取或创建标签
    async fn get_or_create_tag(&self, tag_name: &str) -> Result<String> {
        let slug = slug::generate_slug(tag_name);
        debug!("Getting or creating tag with name: {}, slug: {}", tag_name, slug);
        
        // 查找现有标签
        if let Some(tag) = self.db.find_one::<Value>("tag", "slug", &slug).await? {
            debug!("Found existing tag: {:?}", tag);
            if let Some(id_value) = tag.get("id") {
                match id_value {
                    Value::String(s) => return Ok(s.clone()),
                    Value::Object(obj) => {
                        // 处理 Thing 格式 { "tb": "tag", "id": { "String": "xxx" } }
                        if let (Some(tb), Some(id)) = (obj.get("tb"), obj.get("id")) {
                            let tb_str = tb.as_str().unwrap_or("tag");
                            let id_str = match id {
                                Value::String(s) => s.clone(),
                                Value::Object(id_obj) => {
                                    id_obj.get("String")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown")
                                        .to_string()
                                },
                                _ => id.to_string()
                            };
                            return Ok(format!("{}:{}", tb_str, id_str));
                        }
                    },
                    _ => {}
                }
            }
        }

        // 创建新标签
        let tag_uuid = Uuid::new_v4().to_string();
        let tag_id = format!("tag:{}", tag_uuid);
        debug!("Creating new tag with id: {}", tag_id);
        
        // 使用 CREATE tag:id 语法而不是参数化的 $id
        let query = format!(
            r#"
            CREATE tag:`{}` CONTENT {{
                name: $name,
                slug: $slug,
                follower_count: 0,
                article_count: 0,
                is_featured: false,
                created_at: time::now(),
                updated_at: time::now()
            }} RETURN *
            "#,
            tag_uuid
        );

        let result = self.db.query_with_params(
            &query,
            json!({ 
                "name": tag_name,
                "slug": slug
            })
        ).await?;

        let mut response = result;
        match response.take::<Option<Value>>(0) {
            Ok(Some(created)) => {
                debug!("Tag created successfully: {:?}", created);
                // 我们已经使用了完整的 tag:uuid 格式作为 ID
                return Ok(tag_id);
            },
            Ok(None) => {
                error!("Tag creation returned no results");
                Err(AppError::Internal("Tag creation returned no results".to_string()))
            },
            Err(e) => {
                error!("Failed to parse tag creation result: {:?}", e);
                Err(AppError::Internal(format!("Failed to parse tag creation result: {:?}", e)))
            }
        }
    }

    /// 发布文章
    pub async fn publish_article(&self, article_id: &str, author_id: &str) -> Result<Article> {
        debug!("Publishing article: {} by user: {}", article_id, author_id);
        
        // 获取文章
        let mut article = self.get_article_by_id(article_id).await?
            .ok_or_else(|| AppError::NotFound("Article not found".to_string()))?;
        
        // 检查权限
        if article.author_id != author_id {
            return Err(AppError::Authorization("Only article author can publish this article".to_string()));
        }
        
        // 检查是否已发布
        if article.status == ArticleStatus::Published {
            return Err(AppError::BadRequest("Article is already published".to_string()));
        }
        
        // 使用 UPDATE 查询而不是对象更新，避免 ID 格式问题
        let id_without_prefix = if article_id.starts_with("article:") {
            &article_id[8..]
        } else {
            article_id
        };
        
        let update_query = format!(
            "UPDATE article:`{}` SET status = $status, published_at = time::now(), updated_at = time::now() RETURN *",
            id_without_prefix
        );
        
        let mut response = self.db.query_with_params(&update_query, json!({
            "status": "published"
        })).await?;
        
        let updated_articles: Vec<Article> = response.take(0)?;
        let updated_article = updated_articles.into_iter().next()
            .ok_or_else(|| AppError::NotFound("Failed to publish article".to_string()))?;
        
        info!("Published article: {}", article_id);
        Ok(updated_article)
    }
    
    /// 取消发布文章
    pub async fn unpublish_article(&self, article_id: &str, author_id: &str) -> Result<Article> {
        debug!("Unpublishing article: {} by user: {}", article_id, author_id);
        
        // 获取文章
        let mut article = self.get_article_by_id(article_id).await?
            .ok_or_else(|| AppError::NotFound("Article not found".to_string()))?;
        
        // 检查权限
        if article.author_id != author_id {
            return Err(AppError::Authorization("Only article author can unpublish this article".to_string()));
        }
        
        // 检查是否已是草稿
        if article.status == ArticleStatus::Draft {
            return Err(AppError::BadRequest("Article is already in draft status".to_string()));
        }
        
        // 使用 UPDATE 查询而不是对象更新，避免 ID 格式问题
        let id_without_prefix = if article_id.starts_with("article:") {
            &article_id[8..]
        } else {
            article_id
        };
        
        let update_query = format!(
            "UPDATE article:`{}` SET status = $status, updated_at = time::now() RETURN *",
            id_without_prefix
        );
        
        let mut response = self.db.query_with_params(&update_query, json!({
            "status": "draft"
        })).await?;
        
        let updated_articles: Vec<Article> = response.take(0)?;
        let updated_article = updated_articles.into_iter().next()
            .ok_or_else(|| AppError::NotFound("Failed to unpublish article".to_string()))?;
        
        info!("Unpublished article: {}", article_id);
        Ok(updated_article)
    }

    /// 聚合每日统计
    pub async fn aggregate_daily_stats(&self) -> Result<()> {
        debug!("Aggregating daily article stats");

        // 使用更简单的方法来避免复杂的字段名
        let today = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
        let tomorrow = today + chrono::Duration::days(1);
        
        // 先获取统计数据
        let stats_query = r#"
            SELECT 
                count() as total_articles,
                math::sum(view_count) as total_views,
                math::sum(clap_count) as total_claps,
                math::sum(comment_count) as total_comments,
                math::mean(reading_time) as avg_reading_time
            FROM article
            WHERE created_at >= $today 
            AND created_at < $tomorrow
        "#;
        
        let mut response = self.db.query_with_params(stats_query, json!({
            "today": today,
            "tomorrow": tomorrow
        })).await?;
        
        let stats: Vec<serde_json::Value> = response.take(0)?;
        
        if let Some(stat) = stats.first() {
            // 创建或更新统计记录
            let upsert_query = r#"
                UPDATE daily_article_stats:[$today] MERGE $stats
            "#;
            
            let stats_data = json!({
                "date": today,
                "total_articles": stat.get("total_articles").and_then(|v| v.as_i64()).unwrap_or(0),
                "total_views": stat.get("total_views").and_then(|v| v.as_i64()).unwrap_or(0),
                "total_claps": stat.get("total_claps").and_then(|v| v.as_i64()).unwrap_or(0),
                "total_comments": stat.get("total_comments").and_then(|v| v.as_i64()).unwrap_or(0),
                "avg_reading_time": stat.get("avg_reading_time").and_then(|v| v.as_f64()).unwrap_or(0.0),
                "updated_at": Utc::now()
            });
            
            self.db.query_with_params(upsert_query, json!({
                "today": today.to_string(),
                "stats": stats_data
            })).await?;
        }
        
        Ok(())
    }

    /// 获取文章作者信息
    async fn get_article_author(&self, author_id: &str) -> Result<AuthorInfo> {
        debug!("Getting author info for: {}", author_id);

        let query = r#"
            SELECT id, username, display_name, avatar_url, is_verified 
            FROM user_profile 
            WHERE user_id = $author_id
            LIMIT 1
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "author_id": author_id
        })).await?;

        // 直接获取原始数据
        let results: Vec<Value> = response.take(0)?;
        let author_data = results.into_iter().next()
            .ok_or_else(|| AppError::NotFound(format!("Author {} not found", author_id)))?;

        // 手动构造 AuthorInfo
        Ok(AuthorInfo {
            id: author_data.get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            username: author_data.get("username")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            display_name: author_data.get("display_name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            avatar_url: author_data.get("avatar_url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            is_verified: author_data.get("is_verified")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        })
    }

    /// 获取文章标签
    async fn get_article_tags(&self, article_id: &str) -> Result<Vec<TagInfo>> {
        debug!("Getting tags for article: {}", article_id);

        // First get article_tag relationships
        let query = r#"
            SELECT tag_id FROM article_tag WHERE article_id = $article_id
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "article_id": article_id
        })).await?;

        let tag_relations: Vec<Value> = response.take(0).unwrap_or_default();
        
        if tag_relations.is_empty() {
            return Ok(Vec::new());
        }

        // Extract tag IDs
        let tag_ids: Vec<String> = tag_relations.iter()
            .filter_map(|v| v.get("tag_id").and_then(|id| id.as_str()))
            .map(|s| s.to_string())
            .collect();

        if tag_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Now get the tags
        let mut tags = Vec::new();
        for tag_id in tag_ids {
            let tag_query = "SELECT * FROM $tag_id";
            if let Ok(mut tag_response) = self.db.query_with_params(tag_query, json!({
                "tag_id": tag_id
            })).await {
                if let Ok(tag_values) = tag_response.take::<Vec<Value>>(0) {
                    for tag_value in tag_values {
                        if let Ok(tag_info) = serde_json::from_value::<TagInfo>(tag_value) {
                            tags.push(tag_info);
                        }
                    }
                }
            }
        }

        tags.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(tags)
    }

    /// 获取文章出版物信息
    async fn get_article_publication(&self, publication_id: &str) -> Result<Option<PublicationInfo>> {
        debug!("Getting publication info for: {}", publication_id);

        let query = r#"
            SELECT id, name, slug, logo_url 
            FROM publication 
            WHERE id = $publication_id
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "publication_id": publication_id
        })).await?;

        let publications: Vec<PublicationInfo> = response.take(0)?;
        Ok(publications.into_iter().next())
    }

    /// 获取文章系列信息
    async fn get_article_series(&self, series_id: &str, article_id: &str) -> Result<Option<SeriesInfo>> {
        debug!("Getting series info for: {}", series_id);

        // First get the series
        let series_query = r#"
            SELECT id, title, slug FROM series WHERE id = $series_id LIMIT 1
        "#;

        let mut response = self.db.query_with_params(series_query, json!({
            "series_id": series_id
        })).await?;

        let series_data: Vec<Value> = response.take(0)?;
        if let Some(series) = series_data.into_iter().next() {
            // Then get the order from series_article
            let order_query = r#"
                SELECT `order` FROM series_article 
                WHERE series_id = $series_id AND article_id = $article_id
                LIMIT 1
            "#;
            
            let mut order_response = self.db.query_with_params(order_query, json!({
                "series_id": series_id,
                "article_id": article_id
            })).await?;
            
            let order_data: Vec<Value> = order_response.take(0)?;
            let order = order_data.into_iter().next()
                .and_then(|v| v.get("order").and_then(|o| o.as_i64()))
                .unwrap_or(0) as i32;
            
            Ok(Some(SeriesInfo {
                id: series.get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                title: series.get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                slug: series.get("slug")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                order,
            }))
        } else {
            Ok(None)
        }
    }

    /// 检查用户是否收藏了文章
    async fn is_article_bookmarked(&self, article_id: &str, user_id: &str) -> Result<bool> {
        let query = r#"
            SELECT count() as count 
            FROM bookmark 
            WHERE article_id = $article_id AND user_id = $user_id AND is_deleted = false
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "article_id": article_id,
            "user_id": user_id
        })).await?;

        let result: Vec<serde_json::Value> = response.take(0)?;
        let count = result.first()
            .and_then(|v| v.get("count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        Ok(count > 0)
    }

    /// 检查用户是否点赞了文章
    async fn is_article_clapped(&self, article_id: &str, user_id: &str) -> Result<bool> {
        let query = r#"
            SELECT count() as count 
            FROM clap 
            WHERE article_id = $article_id AND user_id = $user_id
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "article_id": article_id,
            "user_id": user_id
        })).await?;

        let result: Vec<serde_json::Value> = response.take(0)?;
        let count = result.first()
            .and_then(|v| v.get("count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        Ok(count > 0)
    }

    /// 获取用户对文章的点赞次数
    async fn get_user_clap_count(&self, article_id: &str, user_id: &str) -> Result<i32> {
        let query = r#"
            SELECT count 
            FROM clap 
            WHERE article_id = $article_id AND user_id = $user_id
            LIMIT 1
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "article_id": article_id,
            "user_id": user_id
        })).await?;

        let result: Vec<serde_json::Value> = response.take(0)?;
        let count = result.first()
            .and_then(|v| v.get("count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        Ok(count)
    }

    /// 为文章添加点赞
    pub async fn clap_article(&self, article_id: &str, user_id: &str, count: i32) -> Result<crate::models::clap::ClapResponse> {
        debug!("User {} clapping article {} with count {}", user_id, article_id, count);

        // 验证文章存在且已发布
        let article = self.get_article_by_id(article_id).await
            .map_err(|e| {
                error!("Failed to get article by id {}: {:?}", article_id, e);
                e
            })?
            .ok_or_else(|| {
                error!("Article not found: {}", article_id);
                AppError::NotFound("Article not found".to_string())
            })?;

        if article.status != ArticleStatus::Published {
            return Err(AppError::forbidden("Cannot clap unpublished articles"));
        }

        // 获取用户现有的点赞
        let query = format!(r#"
            SELECT meta::tb(id) as tb, meta::id(id) as id_val, count FROM clap 
            WHERE user_id = $user_id 
            AND article_id = article:`{}`
        "#, article_id);
        
        debug!("Querying existing claps with user_id: {} and article_id: {}", user_id, article_id);
        
        let mut response = self.db
            .query_with_params(&query, json!({
                "user_id": user_id
            }))
            .await
            .map_err(|e| {
                error!("Failed to query existing claps: {:?}", e);
                e
            })?;
        let clap_data: Vec<Value> = response.take(0)?;
        let existing_clap = clap_data.into_iter().next();

        let user_clap_count = if let Some(clap_value) = existing_clap {
            // 获取现有点赞数
            let current_count = clap_value.get("count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
            
            // 检查总数是否超过50
            let new_total = current_count + count;
            if new_total > 50 {
                return Err(AppError::BadRequest(
                    format!("Maximum claps per article is 50. You have {} claps already.", current_count)
                ));
            }

            // 获取点赞ID - 使用meta函数返回的值
            let tb = clap_value.get("tb")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AppError::internal("Missing table name"))?;
            let id_val = clap_value.get("id_val")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AppError::internal("Missing ID value"))?;
            
            let clap_id = format!("{}:{}", tb, id_val);
            debug!("Updating clap with ID: {}", clap_id);

            // 更新现有点赞 - 使用反引号包裹ID
            let update_query = format!(
                "UPDATE clap:`{}` SET count = $count, updated_at = time::now() RETURN count",
                id_val
            );
            
            let mut update_response = self.db.query_with_params(&update_query, json!({
                "count": new_total
            })).await?;
            
            let result: Vec<Value> = update_response.take(0)?;
            result.into_iter().next()
                .and_then(|v| v.get("count").and_then(|c| c.as_i64()))
                .unwrap_or(new_total as i64) as i32
        } else {
            // 创建新点赞
            if count > 50 {
                return Err(AppError::BadRequest("Maximum claps per article is 50".to_string()));
            }

            // 使用 SQL 创建点赞记录，article_id 使用 record 类型
            let clap_id = Uuid::new_v4().to_string();
            let create_query = format!(r#"
                CREATE clap:`{}` CONTENT {{
                    user_id: $user_id,
                    article_id: article:`{}`,
                    count: $count,
                    created_at: time::now(),
                    updated_at: time::now()
                }}
            "#, clap_id, article_id);
            
            let mut create_response = self.db.query_with_params(&create_query, json!({
                "user_id": user_id,
                "count": count
            })).await?;
            
            // 检查创建是否成功
            let created_results: Vec<Value> = create_response.take(0)?;
            debug!("Created clap results: {:?}", created_results);
            
            count
        };

        // 更新文章总点赞数
        debug!("Updating article clap count for article_id: {}", article_id);
        self.update_article_clap_count(article_id).await?;

        // 获取文章最新的总点赞数
        debug!("Getting total claps for article_id: {}", article_id);
        let total_claps = self.get_article_total_claps(article_id).await?;

        Ok(crate::models::clap::ClapResponse {
            user_clap_count,
            total_claps,
        })
    }

    /// 更新文章的总点赞数
    async fn update_article_clap_count(&self, article_id: &str) -> Result<()> {
        // 获取所有点赞记录的count值
        let count_query = format!(
            "SELECT count FROM clap WHERE article_id = article:`{}`",
            article_id
        );
        
        debug!("Getting all clap counts for article: {}", article_id);
        
        let mut count_response = self.db.query(&count_query).await?;
        let clap_records: Vec<Value> = count_response.take(0)?;
        
        // 在应用层计算总和
        let total_claps: i64 = clap_records.iter()
            .filter_map(|v| v.get("count"))
            .filter_map(|v| v.as_i64())
            .sum();
        
        debug!("Total claps calculated for article {}: {}", article_id, total_claps);
        
        // 更新文章的点赞数
        let update_query = format!(
            "UPDATE article:`{}` SET clap_count = {}",
            article_id, total_claps
        );
        
        debug!("Updating article clap_count with query: {}", update_query);
        
        self.db.query(&update_query).await?;
        
        info!("Successfully updated article {} clap_count to {}", article_id, total_claps);

        Ok(())
    }

    /// 获取文章的总点赞数
    async fn get_article_total_claps(&self, article_id: &str) -> Result<i64> {
        let query = format!("SELECT clap_count FROM article:`{}`", article_id);

        let mut response = self.db.query(&query).await?;

        let result: Vec<serde_json::Value> = response.take(0)?;
        let count = result.first()
            .and_then(|v| v.get("clap_count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        Ok(count)
    }

    /// 获取出版物的文章列表
    pub async fn get_articles_by_publication(
        &self, 
        publication_id: &str, 
        page: usize, 
        per_page: usize, 
        tag: Option<&str>,
        search: Option<&str>
    ) -> Result<Vec<ArticleListItem>> {
        debug!("Getting articles for publication: {}", publication_id);
        
        let offset = (page - 1) * per_page;
        
        // 构建查询条件
        let mut conditions = vec![
            "publication_id = $publication_id".to_string(),
            "status = 'published'".to_string(),
            "is_deleted = false".to_string(),
        ];
        
        // 添加标签过滤
        if let Some(tag) = tag {
            conditions.push(format!("$tag IN tags"));
        }
        
        // 添加搜索过滤
        if let Some(search_term) = search {
            conditions.push(format!("(title ~ $search OR content ~ $search)"));
        }
        
        let where_clause = conditions.join(" AND ");
        
        let query = format!(r#"
            SELECT 
                id, title, subtitle, slug, excerpt, cover_image_url,
                author_id, publication_id, reading_time, 
                view_count, clap_count, comment_count,
                created_at, published_at
            FROM article 
            WHERE {}
            ORDER BY published_at DESC
            LIMIT $limit START $offset
        "#, where_clause);
        
        let mut params = json!({
            "publication_id": publication_id,
            "limit": per_page,
            "offset": offset
        });
        
        if let Some(tag) = tag {
            params["tag"] = json!(tag);
        }
        
        if let Some(search_term) = search {
            params["search"] = json!(search_term);
        }
        
        let mut response = self.db.query_with_params(&query, params).await?;
        let articles: Vec<ArticleListItem> = response.take(0)?;
        
        Ok(articles)
    }
    
    /// 统计出版物的文章总数
    pub async fn count_articles_by_publication(
        &self, 
        publication_id: &str,
        tag: Option<&str>,
        search: Option<&str>
    ) -> Result<usize> {
        debug!("Counting articles for publication: {}", publication_id);
        
        // 构建查询条件
        let mut conditions = vec![
            "publication_id = $publication_id".to_string(),
            "status = 'published'".to_string(),
            "is_deleted = false".to_string(),
        ];
        
        // 添加标签过滤
        if let Some(tag) = tag {
            conditions.push(format!("$tag IN tags"));
        }
        
        // 添加搜索过滤
        if let Some(search_term) = search {
            conditions.push(format!("(title ~ $search OR content ~ $search)"));
        }
        
        let where_clause = conditions.join(" AND ");
        
        let query = format!(r#"
            SELECT count() as total FROM article 
            WHERE {}
        "#, where_clause);
        
        let mut params = json!({
            "publication_id": publication_id
        });
        
        if let Some(tag) = tag {
            params["tag"] = json!(tag);
        }
        
        if let Some(search_term) = search {
            params["search"] = json!(search_term);
        }
        
        let mut response = self.db.query_with_params(&query, params).await?;
        
        let result: Vec<serde_json::Value> = response.take(0)?;
        let count = result.first()
            .and_then(|v| v.get("total"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as usize;
        
        Ok(count)
    }

    /// 获取出版物中特定slug的文章
    pub async fn get_article_by_slug_in_publication(
        &self,
        publication_id: &str,
        slug: &str,
        viewer_user_id: Option<&str>
    ) -> Result<Option<ArticleResponse>> {
        debug!("Getting article by slug {} in publication {}", slug, publication_id);
        
        // 获取文章基础信息并检查是否属于该出版物
        let article = match self.get_article_by_slug(slug).await? {
            Some(article) => article,
            None => return Ok(None),
        };
        
        // 检查文章是否属于该出版物
        if article.publication_id.as_deref() != Some(publication_id) {
            return Ok(None);
        }
        
        // 获取完整的文章信息
        self.get_article_with_details(slug, viewer_user_id).await
    }
    
    /// 获取出版物中的相关文章
    pub async fn get_related_articles_in_publication(
        &self,
        publication_id: &str,
        article_id: &str,
        limit: usize
    ) -> Result<Vec<ArticleListItem>> {
        debug!("Getting related articles for {} in publication {}", article_id, publication_id);
        
        // 获取当前文章的标签
        let tags = self.get_article_tags(article_id).await?;
        let tag_ids: Vec<String> = tags.iter().map(|t| t.id.clone()).collect();
        
        if tag_ids.is_empty() {
            // 如果没有标签，返回该出版物最新的文章
            return self.get_articles_by_publication(publication_id, 1, limit, None, None).await;
        }
        
        // 基于标签查找相关文章
        let query = r#"
            SELECT DISTINCT
                a.id, a.title, a.subtitle, a.slug, a.excerpt, a.cover_image_url,
                a.author_id, a.publication_id, a.reading_time, 
                a.view_count, a.clap_count, a.comment_count,
                a.created_at, a.published_at
            FROM article a
            JOIN article_tag at ON a.id = at.article_id
            WHERE a.publication_id = $publication_id
                AND a.id != $article_id
                AND at.tag_id IN $tag_ids
                AND a.status = 'published'
                AND a.is_deleted = false
            ORDER BY a.published_at DESC
            LIMIT $limit
        "#;
        
        let mut response = self.db.query_with_params(query, json!({
            "publication_id": publication_id,
            "article_id": article_id,
            "tag_ids": tag_ids,
            "limit": limit
        })).await?;
        
        Ok(response.take(0)?)
    }
    
    /// 获取出版物中特定用户的文章数量
    pub async fn count_articles_by_user_in_publication(
        &self,
        publication_id: &str,
        user_id: &str
    ) -> Result<usize> {
        debug!("Counting articles by user {} in publication {}", user_id, publication_id);
        
        let query = r#"
            SELECT count() as total 
            FROM article 
            WHERE publication_id = $publication_id 
                AND author_id = $user_id
                AND status = 'published' 
                AND is_deleted = false
        "#;
        
        let mut response = self.db.query_with_params(query, json!({
            "publication_id": publication_id,
            "user_id": user_id
        })).await?;
        
        let result: Vec<serde_json::Value> = response.take(0)?;
        let count = result.first()
            .and_then(|v| v.get("total"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as usize;
        
        Ok(count)
    }
    
    /// 获取出版物的总浏览量
    pub async fn get_total_views_by_publication(&self, publication_id: &str) -> Result<usize> {
        debug!("Getting total views for publication {}", publication_id);
        
        let query = r#"
            SELECT math::sum(view_count) as total_views 
            FROM article 
            WHERE publication_id = $publication_id 
                AND status = 'published' 
                AND is_deleted = false
        "#;
        
        let mut response = self.db.query_with_params(query, json!({
            "publication_id": publication_id
        })).await?;
        
        let result: Vec<serde_json::Value> = response.take(0)?;
        let count = result.first()
            .and_then(|v| v.get("total_views"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as usize;
        
        Ok(count)
    }
    
    /// Helper method to convert article data to ArticleListItem
    async fn article_to_list_item(&self, article: &Article) -> Result<ArticleListItem> {
        // Get author info
        let author_query = r#"
            SELECT id, username, display_name, avatar_url, is_verified
            FROM user_profile
            WHERE user_id = $author_id
        "#;
        
        let mut author_response = self.db.query_with_params(author_query, json!({
            "author_id": &article.author_id
        })).await?;
        
        let author_data: Vec<Value> = author_response.take(0)?;
        let author_info = if let Some(author) = author_data.first() {
            AuthorInfo {
                id: author["id"].as_str().unwrap_or("").to_string(),
                username: author["username"].as_str().unwrap_or("").to_string(),
                display_name: author["display_name"].as_str().unwrap_or("").to_string(),
                avatar_url: author["avatar_url"].as_str().map(String::from),
                is_verified: author["is_verified"].as_bool().unwrap_or(false),
            }
        } else {
            AuthorInfo {
                id: article.author_id.clone(),
                username: "unknown".to_string(),
                display_name: "Unknown Author".to_string(),
                avatar_url: None,
                is_verified: false,
            }
        };
        
        // Get publication info if exists
        let publication_info = if let Some(pub_id) = &article.publication_id {
            let pub_query = r#"
                SELECT id, name, slug, logo_url
                FROM publication
                WHERE id = $publication_id
            "#;
            
            let mut pub_response = self.db.query_with_params(pub_query, json!({
                "publication_id": pub_id
            })).await?;
            
            let pub_data: Vec<Value> = pub_response.take(0)?;
            pub_data.first().map(|p| PublicationInfo {
                id: p["id"].as_str().unwrap_or("").to_string(),
                name: p["name"].as_str().unwrap_or("").to_string(),
                slug: p["slug"].as_str().unwrap_or("").to_string(),
                logo_url: p["logo_url"].as_str().map(String::from),
            })
        } else {
            None
        };
        
        // Get tags info - 先获取article_tag关系，再获取tag详情
        let tag_relations_query = "SELECT tag_id FROM article_tag WHERE article_id = $article_id";
        
        let mut tag_rel_response = self.db.query_with_params(tag_relations_query, json!({
            "article_id": &article.id
        })).await?;
        
        let tag_relations: Vec<Value> = tag_rel_response.take(0)?;
        let mut tags: Vec<TagInfo> = Vec::new();
        
        for rel in tag_relations {
            if let Some(tag_id) = rel.get("tag_id").and_then(|v| v.as_str()) {
                // 获取tag详情
                if let Ok(mut tag_response) = self.db.query(&format!("SELECT * FROM {}", tag_id)).await {
                    if let Ok(tag_values) = tag_response.take::<Vec<Value>>(0) {
                        if let Some(tag_value) = tag_values.first() {
                            tags.push(TagInfo {
                                id: tag_value.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                name: tag_value.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                slug: tag_value.get("slug").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            });
                        }
                    }
                }
            }
        }
        
        Ok(ArticleListItem {
            id: article.id.clone(),
            title: article.title.clone(),
            subtitle: article.subtitle.clone(),
            slug: article.slug.clone(),
            excerpt: article.excerpt.clone(),
            cover_image_url: article.cover_image_url.clone(),
            author: author_info,
            publication: publication_info,
            status: article.status.clone(),
            is_paid_content: article.is_paid_content,
            is_featured: article.is_featured,
            reading_time: article.reading_time,
            view_count: article.view_count,
            clap_count: article.clap_count,
            comment_count: article.comment_count,
            tags,
            created_at: article.created_at,
            published_at: article.published_at,
        })
    }
}