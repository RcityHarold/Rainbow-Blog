use crate::{
    error::{AppError, Result},
    models::search::*,
    services::Database,
};
use chrono::{Utc, DateTime, Duration};
use serde_json::{json, Value};
use std::{sync::Arc, collections::HashMap};
use tracing::{debug, info};
use validator::Validate;

#[derive(Clone)]
pub struct SearchService {
    db: Arc<Database>,
}

impl SearchService {
    pub async fn new(db: Arc<Database>) -> Result<Self> {
        Ok(Self { db })
    }

    pub async fn search(&self, query: SearchQuery) -> Result<SearchResults> {
        debug!("Searching for: {}", query.q);

        let search_term = query.q.trim();
        if search_term.is_empty() {
            return Ok(SearchResults {
                articles: vec![],
                users: vec![],
                tags: vec![],
                publications: vec![],
                total_results: 0,
            });
        }

        let page = query.page.unwrap_or(1).max(1);
        let limit = query.limit.unwrap_or(10).min(50);
        let search_type = query.search_type.unwrap_or(SearchType::All);

        let mut results = SearchResults {
            articles: vec![],
            users: vec![],
            tags: vec![],
            publications: vec![],
            total_results: 0,
        };

        match search_type {
            SearchType::All => {
                // 搜索所有类型，每种类型限制数量
                results.articles = self.search_articles(search_term, 1, 5).await?;
                results.users = self.search_users(search_term, 1, 5).await?;
                results.tags = self.search_tags(search_term, 1, 5).await?;
                results.publications = self.search_publications(search_term, 1, 5).await?;
                
                results.total_results = (results.articles.len() 
                    + results.users.len() 
                    + results.tags.len() 
                    + results.publications.len()) as i64;
            }
            SearchType::Articles => {
                results.articles = self.search_articles(search_term, page, limit).await?;
                results.total_results = results.articles.len() as i64;
            }
            SearchType::Users => {
                results.users = self.search_users(search_term, page, limit).await?;
                results.total_results = results.users.len() as i64;
            }
            SearchType::Tags => {
                results.tags = self.search_tags(search_term, page, limit).await?;
                results.total_results = results.tags.len() as i64;
            }
            SearchType::Publications => {
                results.publications = self.search_publications(search_term, page, limit).await?;
                results.total_results = results.publications.len() as i64;
            }
        }

        Ok(results)
    }

    async fn search_articles(&self, search_term: &str, page: i32, limit: i32) -> Result<Vec<ArticleSearchResult>> {
        let offset = (page - 1) * limit;

        let query = r#"
            SELECT 
                a.id,
                a.title,
                a.slug,
                a.excerpt,
                a.cover_image_url,
                a.reading_time,
                a.published_at,
                a.clap_count,
                a.comment_count,
                u.display_name as author_name,
                u.username as author_username
            FROM article a
            JOIN user_profile u ON a.author_id = u.user_id
            WHERE a.status = 'published'
            AND a.is_deleted = false
            AND (
                a.title CONTAINS $search_term
                OR a.content CONTAINS $search_term
                OR a.excerpt CONTAINS $search_term
                OR u.display_name CONTAINS $search_term
                OR u.username CONTAINS $search_term
            )
            ORDER BY a.popularity_score DESC, a.published_at DESC
            LIMIT $limit
            START $offset
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "search_term": search_term,
            "limit": limit,
            "offset": offset
        })).await?;
        let articles: Vec<Value> = response.take(0)?;

        let mut results = Vec::new();
        for article_data in articles {
            let article_id = article_data["id"].as_str().unwrap_or("");
            
            // 获取文章标签
            let tags = self.get_article_tags(article_id).await?;

            let mut article_result: ArticleSearchResult = serde_json::from_value(article_data)?;
            article_result.tags = tags;
            
            // 添加搜索高亮（简化版本）
            if article_result.title.to_lowercase().contains(&search_term.to_lowercase()) {
                article_result.highlight = Some(SearchHighlight {
                    field: "title".to_string(),
                    snippet: self.create_highlight_snippet(&article_result.title, search_term),
                });
            } else if let Some(ref excerpt) = article_result.excerpt {
                if excerpt.to_lowercase().contains(&search_term.to_lowercase()) {
                    article_result.highlight = Some(SearchHighlight {
                        field: "excerpt".to_string(),
                        snippet: self.create_highlight_snippet(excerpt, search_term),
                    });
                }
            }

            results.push(article_result);
        }

        Ok(results)
    }

    async fn search_users(&self, search_term: &str, page: i32, limit: i32) -> Result<Vec<UserSearchResult>> {
        let offset = (page - 1) * limit;

        let query = r#"
            SELECT 
                user_id,
                username,
                display_name,
                avatar_url,
                bio,
                is_verified,
                follower_count,
                article_count
            FROM user_profile
            WHERE is_suspended = false
            AND (
                username CONTAINS $search_term
                OR display_name CONTAINS $search_term
                OR bio CONTAINS $search_term
            )
            ORDER BY follower_count DESC, article_count DESC
            LIMIT $limit
            START $offset
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "search_term": search_term,
            "limit": limit,
            "offset": offset
        })).await?;
        let users: Vec<Value> = response.take(0)?;

        let mut results = Vec::new();
        for user_data in users {
            let mut user_result: UserSearchResult = serde_json::from_value(user_data)?;
            
            // 添加搜索高亮
            if user_result.username.to_lowercase().contains(&search_term.to_lowercase()) {
                user_result.highlight = Some(SearchHighlight {
                    field: "username".to_string(),
                    snippet: self.create_highlight_snippet(&user_result.username, search_term),
                });
            } else if user_result.display_name.to_lowercase().contains(&search_term.to_lowercase()) {
                user_result.highlight = Some(SearchHighlight {
                    field: "display_name".to_string(),
                    snippet: self.create_highlight_snippet(&user_result.display_name, search_term),
                });
            } else if let Some(ref bio) = user_result.bio {
                if bio.to_lowercase().contains(&search_term.to_lowercase()) {
                    user_result.highlight = Some(SearchHighlight {
                        field: "bio".to_string(),
                        snippet: self.create_highlight_snippet(bio, search_term),
                    });
                }
            }

            results.push(user_result);
        }

        Ok(results)
    }

    async fn search_tags(&self, search_term: &str, page: i32, limit: i32) -> Result<Vec<TagSearchResult>> {
        let offset = (page - 1) * limit;

        let query = r#"
            SELECT 
                id,
                name,
                slug,
                description,
                article_count,
                follower_count,
                is_featured
            FROM tag
            WHERE name CONTAINS $search_term
            OR description CONTAINS $search_term
            ORDER BY article_count DESC, follower_count DESC
            LIMIT $limit
            START $offset
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "search_term": search_term,
            "limit": limit,
            "offset": offset
        })).await?;
        let tags: Vec<Value> = response.take(0)?;

        let mut results = Vec::new();
        for tag_data in tags {
            let mut tag_result: TagSearchResult = serde_json::from_value(tag_data)?;
            
            // 添加搜索高亮
            if tag_result.name.to_lowercase().contains(&search_term.to_lowercase()) {
                tag_result.highlight = Some(SearchHighlight {
                    field: "name".to_string(),
                    snippet: self.create_highlight_snippet(&tag_result.name, search_term),
                });
            } else if let Some(ref description) = tag_result.description {
                if description.to_lowercase().contains(&search_term.to_lowercase()) {
                    tag_result.highlight = Some(SearchHighlight {
                        field: "description".to_string(),
                        snippet: self.create_highlight_snippet(description, search_term),
                    });
                }
            }

            results.push(tag_result);
        }

        Ok(results)
    }

    async fn search_publications(&self, search_term: &str, page: i32, limit: i32) -> Result<Vec<PublicationSearchResult>> {
        let offset = (page - 1) * limit;

        let query = r#"
            SELECT 
                id,
                name,
                slug,
                description,
                tagline,
                logo_url,
                member_count,
                article_count,
                follower_count
            FROM publication
            WHERE is_suspended = false
            AND (
                name CONTAINS $search_term
                OR description CONTAINS $search_term
                OR tagline CONTAINS $search_term
            )
            ORDER BY follower_count DESC, article_count DESC
            LIMIT $limit
            START $offset
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "search_term": search_term,
            "limit": limit,
            "offset": offset
        })).await?;
        let publications: Vec<Value> = response.take(0)?;

        let mut results = Vec::new();
        for pub_data in publications {
            let mut pub_result: PublicationSearchResult = serde_json::from_value(pub_data)?;
            
            // 添加搜索高亮
            if pub_result.name.to_lowercase().contains(&search_term.to_lowercase()) {
                pub_result.highlight = Some(SearchHighlight {
                    field: "name".to_string(),
                    snippet: self.create_highlight_snippet(&pub_result.name, search_term),
                });
            } else if let Some(ref description) = pub_result.description {
                if description.to_lowercase().contains(&search_term.to_lowercase()) {
                    pub_result.highlight = Some(SearchHighlight {
                        field: "description".to_string(),
                        snippet: self.create_highlight_snippet(description, search_term),
                    });
                }
            }

            results.push(pub_result);
        }

        Ok(results)
    }

    pub async fn get_search_suggestions(&self, query: &str, limit: Option<i32>) -> Result<Vec<SearchSuggestion>> {
        debug!("Getting search suggestions for: {}", query);

        let limit = limit.unwrap_or(10).min(20);
        let mut suggestions = Vec::new();

        // 获取热门搜索词建议
        let popular_searches = self.get_popular_searches(query, limit / 2).await?;
        for search in popular_searches {
            suggestions.push(SearchSuggestion {
                text: search,
                suggestion_type: SuggestionType::Query,
                metadata: None,
            });
        }

        // 获取标签建议
        let tag_suggestions = self.get_tag_suggestions(query, limit / 2).await?;
        for tag in tag_suggestions {
            suggestions.push(SearchSuggestion {
                text: tag.name.clone(),
                suggestion_type: SuggestionType::Tag,
                metadata: Some(json!({
                    "slug": tag.slug,
                    "article_count": tag.article_count
                })),
            });
        }

        Ok(suggestions)
    }

    pub async fn update_search_index(&self, article_id: &str) -> Result<()> {
        debug!("Updating search index for article: {}", article_id);

        let query = r#"
            LET $article = (SELECT * FROM article WHERE id = $article_id);
            LET $author = (SELECT display_name FROM user_profile WHERE user_id = $article.author_id);
            LET $tags = (SELECT name FROM tag JOIN article_tag ON tag.id = article_tag.tag_id WHERE article_tag.article_id = $article_id);
            LET $publication = (SELECT name FROM publication WHERE id = $article.publication_id);
            
            UPSERT search_index:[$article_id]
            CONTENT {
                article_id: $article_id,
                title: $article.title,
                content: $article.content,
                author_name: $author[0].display_name,
                tags: $tags.*.name,
                publication_name: $publication[0].name,
                is_published: $article.status == 'published',
                published_at: $article.published_at,
                popularity_score: $article.view_count * 0.1 + $article.clap_count * 1 + $article.comment_count * 2 + $article.bookmark_count * 3,
                updated_at: time::now()
            };
        "#;

        self.db.query_with_params(query, json!({
            "article_id": article_id
        })).await?;

        Ok(())
    }

    async fn get_article_tags(&self, article_id: &str) -> Result<Vec<String>> {
        let query = r#"
            SELECT t.name 
            FROM tag t
            JOIN article_tag at ON t.id = at.tag_id
            WHERE at.article_id = $article_id
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "article_id": article_id
        })).await?;
        let tags: Vec<Value> = response.take(0)?;

        Ok(tags
            .into_iter()
            .filter_map(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
            .collect())
    }

    async fn get_popular_searches(&self, prefix: &str, limit: i32) -> Result<Vec<String>> {
        // 简化版本，实际应该从搜索日志中获取
        let searches = vec![
            "rust programming",
            "web development",
            "machine learning",
            "javascript",
            "python",
            "data science",
            "blockchain",
            "artificial intelligence",
        ];

        Ok(searches
            .into_iter()
            .filter(|s| s.starts_with(&prefix.to_lowercase()))
            .take(limit as usize)
            .map(String::from)
            .collect())
    }

    async fn get_tag_suggestions(&self, prefix: &str, limit: i32) -> Result<Vec<TagSearchResult>> {
        let query = r#"
            SELECT 
                id,
                name,
                slug,
                description,
                article_count,
                follower_count,
                is_featured
            FROM tag
            WHERE name BEGINS WITH $prefix
            ORDER BY article_count DESC
            LIMIT $limit
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "prefix": prefix,
            "limit": limit
        })).await?;
        let tags: Vec<Value> = response.take(0)?;

        let mut results = Vec::new();
        for tag_data in tags {
            let tag_result: TagSearchResult = serde_json::from_value(tag_data)?;
            results.push(tag_result);
        }

        Ok(results)
    }

    fn create_highlight_snippet(&self, text: &str, search_term: &str) -> String {
        let lower_text = text.to_lowercase();
        let lower_term = search_term.to_lowercase();
        
        if let Some(pos) = lower_text.find(&lower_term) {
            let start = pos.saturating_sub(30);
            let end = (pos + search_term.len() + 30).min(text.len());
            
            let mut snippet = String::new();
            if start > 0 {
                snippet.push_str("...");
            }
            snippet.push_str(&text[start..end]);
            if end < text.len() {
                snippet.push_str("...");
            }
            
            // 简单的高亮标记
            snippet.replace(search_term, &format!("<mark>{}</mark>", search_term))
        } else {
            text.chars().take(100).collect::<String>() + "..."
        }
    }

    /// 高级搜索功能
    pub async fn advanced_search(
        &self,
        user_id: Option<&str>,
        query: AdvancedSearchQuery,
    ) -> Result<AdvancedSearchResults> {
        debug!("Advanced search with query: {:?}", query);
        
        query.validate().map_err(|e| AppError::ValidatorError(e))?;
        
        let page = query.page.unwrap_or(1).max(1);
        let limit = query.limit.unwrap_or(20).min(100);
        let offset = (page - 1) * limit;
        let search_type = query.search_type.clone().unwrap_or(SearchType::Articles);
        
        let mut results = AdvancedSearchResults {
            articles: vec![],
            users: vec![],
            tags: vec![],
            publications: vec![],
            series: vec![],
            total_results: 0,
            page,
            total_pages: 0,
            facets: SearchFacets {
                tags: vec![],
                authors: vec![],
                publications: vec![],
                date_ranges: vec![],
                reading_time_ranges: vec![],
            },
        };
        
        match search_type {
            SearchType::All => {
                // 对每种类型进行有限搜索
                if let Some(ref q) = query.q {
                    results.articles = self.advanced_article_search(&query, 1, 5, user_id).await?;
                    results.users = self.search_users(q, 1, 5).await?;
                    results.tags = self.search_tags(q, 1, 5).await?;
                    results.publications = self.search_publications(q, 1, 5).await?;
                    results.series = self.search_series(q, 1, 5).await?;
                }
                
                results.total_results = (results.articles.len() 
                    + results.users.len() 
                    + results.tags.len() 
                    + results.publications.len()
                    + results.series.len()) as i64;
            }
            SearchType::Articles => {
                let (articles, total_count) = self.advanced_article_search_with_count(&query, page, limit, user_id).await?;
                results.articles = articles;
                results.total_results = total_count;
                results.total_pages = ((total_count as f64) / (limit as f64)).ceil() as i32;
                
                // 获取facets
                results.facets = self.get_search_facets(&query, user_id).await?;
            }
            _ => {
                // 其他类型暂时使用基础搜索
                if let Some(ref q) = query.q {
                    match search_type {
                        SearchType::Users => {
                            results.users = self.search_users(q, page, limit).await?;
                            results.total_results = results.users.len() as i64;
                        }
                        SearchType::Tags => {
                            results.tags = self.search_tags(q, page, limit).await?;
                            results.total_results = results.tags.len() as i64;
                        }
                        SearchType::Publications => {
                            results.publications = self.search_publications(q, page, limit).await?;
                            results.total_results = results.publications.len() as i64;
                        }
                        _ => {}
                    }
                }
            }
        }
        
        Ok(results)
    }
    
    /// 高级文章搜索
    async fn advanced_article_search(
        &self,
        query: &AdvancedSearchQuery,
        page: i32,
        limit: i32,
        user_id: Option<&str>,
    ) -> Result<Vec<ArticleSearchResult>> {
        let (articles, _) = self.advanced_article_search_with_count(query, page, limit, user_id).await?;
        Ok(articles)
    }
    
    /// 高级文章搜索（带总数）
    async fn advanced_article_search_with_count(
        &self,
        query: &AdvancedSearchQuery,
        page: i32,
        limit: i32,
        user_id: Option<&str>,
    ) -> Result<(Vec<ArticleSearchResult>, i64)> {
        let offset = (page - 1) * limit;
        
        // 构建查询条件
        let mut where_conditions = vec!["a.status = 'published'".to_string()];
        let mut params = json!({
            "limit": limit,
            "offset": offset
        });
        
        // 文本搜索
        if let Some(ref q) = query.q {
            where_conditions.push("(a.title ~ $q OR a.content ~ $q OR a.excerpt ~ $q)".to_string());
            params["q"] = json!(q);
        }
        
        // 作者筛选
        if let Some(ref author) = query.author {
            where_conditions.push("(u.username = $author OR u.display_name ~ $author)".to_string());
            params["author"] = json!(author);
        }
        
        // 标签筛选
        if let Some(ref tags) = query.tags {
            if !tags.is_empty() {
                where_conditions.push("a.id IN (SELECT article_id FROM article_tag WHERE tag_id IN (SELECT id FROM tag WHERE name IN $tags))".to_string());
                params["tags"] = json!(tags);
            }
        }
        
        // 出版物筛选
        if let Some(ref publication) = query.publication {
            where_conditions.push("p.slug = $publication".to_string());
            params["publication"] = json!(publication);
        }
        
        // 系列筛选
        if let Some(ref series) = query.series {
            where_conditions.push("a.id IN (SELECT article_id FROM series_article WHERE series_id = (SELECT id FROM series WHERE slug = $series))".to_string());
            params["series"] = json!(series);
        }
        
        // 日期范围筛选
        if let Some(ref date_from) = query.date_from {
            where_conditions.push("a.published_at >= $date_from".to_string());
            params["date_from"] = json!(date_from);
        }
        
        if let Some(ref date_to) = query.date_to {
            where_conditions.push("a.published_at <= $date_to".to_string());
            params["date_to"] = json!(date_to);
        }
        
        // 阅读时间筛选
        if let Some(min_reading) = query.min_reading_time {
            where_conditions.push(format!("a.reading_time >= {}", min_reading));
        }
        
        if let Some(max_reading) = query.max_reading_time {
            where_conditions.push(format!("a.reading_time <= {}", max_reading));
        }
        
        // 鼓掌数筛选
        if let Some(min_claps) = query.min_claps {
            where_conditions.push(format!("a.clap_count >= {}", min_claps));
        }
        
        // 特色文章筛选
        if let Some(is_featured) = query.is_featured {
            where_conditions.push(format!("a.is_featured = {}", is_featured));
        }
        
        // 付费内容筛选
        if let Some(is_paid) = query.is_paid {
            where_conditions.push(format!("a.is_paid_content = {}", is_paid));
        }
        
        // 排除已读（需要用户ID）
        if let Some(true) = query.exclude_read {
            if let Some(user_id) = &user_id {
                where_conditions.push(format!(
                    "a.id NOT IN (SELECT article_id FROM user_read_history WHERE user_id = '{}')",
                    user_id
                ));
            }
        }
        
        // 构建排序
        let order_by = match query.sort_by.as_ref().unwrap_or(&SortBy::Relevance) {
            SortBy::Relevance => {
                if query.q.is_some() {
                    "score() DESC, a.published_at DESC"
                } else {
                    "a.popularity_score DESC, a.published_at DESC"
                }
            }
            SortBy::PublishedAt => "a.published_at",
            SortBy::UpdatedAt => "a.updated_at",
            SortBy::ClapCount => "a.clap_count",
            SortBy::CommentCount => "a.comment_count",
            SortBy::ViewCount => "a.view_count",
            SortBy::ReadingTime => "a.reading_time",
            SortBy::Title => "a.title",
            SortBy::AuthorName => "u.display_name",
        };
        
        let sort_order = match query.sort_order.as_ref().unwrap_or(&SortOrder::Desc) {
            SortOrder::Asc => "ASC",
            SortOrder::Desc => "DESC",
        };
        
        let where_clause = if where_conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_conditions.join(" AND "))
        };
        
        // 获取总数
        let count_query = format!(
            r#"
            SELECT count() AS total
            FROM article a
            JOIN user_profile u ON a.author_id = u.user_id
            LEFT JOIN publication p ON a.publication_id = p.id
            {}
            "#,
            where_clause
        );
        
        let mut count_response = self.db.query_with_params(&count_query, &params).await?;
        let total_count = if let Ok(Some(result)) = count_response.take::<Option<Value>>(0) {
            result.get("total").and_then(|v| v.as_i64()).unwrap_or(0)
        } else { 0 };
        
        // 获取文章数据
        let data_query = format!(
            r#"
            SELECT 
                a.id,
                a.title,
                a.slug,
                a.excerpt,
                a.cover_image_url,
                a.reading_time,
                a.published_at,
                a.clap_count,
                a.comment_count,
                u.display_name as author_name,
                u.username as author_username,
                p.name as publication_name,
                p.slug as publication_slug
            FROM article a
            JOIN user_profile u ON a.author_id = u.user_id
            LEFT JOIN publication p ON a.publication_id = p.id
            {}
            ORDER BY {} {}
            LIMIT $limit START $offset
            "#,
            where_clause, order_by, sort_order
        );
        
        let mut response = self.db.query_with_params(&data_query, params).await?;
        let articles: Vec<Value> = response.take(0)?;
        
        let mut results = Vec::new();
        for article_data in articles {
            let article_id = article_data["id"].as_str().unwrap_or("");
            
            // 获取文章标签
            let tags = self.get_article_tags(article_id).await?;
            
            let mut article_result: ArticleSearchResult = serde_json::from_value(article_data)?;
            article_result.tags = tags;
            
            // 添加搜索高亮
            if let Some(ref q) = query.q {
                if article_result.title.to_lowercase().contains(&q.to_lowercase()) {
                    article_result.highlight = Some(SearchHighlight {
                        field: "title".to_string(),
                        snippet: self.create_highlight_snippet(&article_result.title, q),
                    });
                } else if let Some(ref excerpt) = article_result.excerpt {
                    if excerpt.to_lowercase().contains(&q.to_lowercase()) {
                        article_result.highlight = Some(SearchHighlight {
                            field: "excerpt".to_string(),
                            snippet: self.create_highlight_snippet(excerpt, q),
                        });
                    }
                }
            }
            
            results.push(article_result);
        }
        
        Ok((results, total_count))
    }
    
    /// 搜索系列
    async fn search_series(&self, search_term: &str, page: i32, limit: i32) -> Result<Vec<SeriesSearchResult>> {
        let offset = (page - 1) * limit;
        
        let query = r#"
            SELECT 
                s.id,
                s.title,
                s.slug,
                s.description,
                s.article_count,
                s.is_completed,
                s.created_at,
                u.display_name as author_name,
                u.username as author_username
            FROM series s
            JOIN user_profile u ON s.author_id = u.user_id
            WHERE s.is_public = true
            AND (
                s.title ~ $search_term
                OR s.description ~ $search_term
                OR u.display_name ~ $search_term
                OR u.username ~ $search_term
            )
            ORDER BY s.subscriber_count DESC, s.created_at DESC
            LIMIT $limit START $offset
        "#;
        
        let mut response = self.db.query_with_params(query, json!({
            "search_term": search_term,
            "limit": limit,
            "offset": offset
        })).await?;
        
        let series: Vec<Value> = response.take(0)?;
        let mut results = Vec::new();
        
        for series_data in series {
            let mut series_result: SeriesSearchResult = serde_json::from_value(series_data)?;
            
            // 添加搜索高亮
            if series_result.title.to_lowercase().contains(&search_term.to_lowercase()) {
                series_result.highlight = Some(SearchHighlight {
                    field: "title".to_string(),
                    snippet: self.create_highlight_snippet(&series_result.title, search_term),
                });
            } else if let Some(ref desc) = series_result.description {
                if desc.to_lowercase().contains(&search_term.to_lowercase()) {
                    series_result.highlight = Some(SearchHighlight {
                        field: "description".to_string(),
                        snippet: self.create_highlight_snippet(desc, search_term),
                    });
                }
            }
            
            results.push(series_result);
        }
        
        Ok(results)
    }
    
    /// 获取搜索 facets
    async fn get_search_facets(
        &self,
        query: &AdvancedSearchQuery,
        user_id: Option<&str>,
    ) -> Result<SearchFacets> {
        let mut facets = SearchFacets {
            tags: vec![],
            authors: vec![],
            publications: vec![],
            date_ranges: vec![],
            reading_time_ranges: vec![],
        };
        
        // 基本查询条件（去掉特定的筛选条件以获取facet计数）
        let base_conditions = vec!["a.status = 'published'".to_string()];
        let base_where = format!("WHERE {}", base_conditions.join(" AND "));
        
        // 获取热门标签
        let tag_query = format!(
            r#"
            SELECT t.name as value, t.name as label, COUNT(DISTINCT a.id) as count
            FROM article a
            JOIN article_tag at ON a.id = at.article_id
            JOIN tag t ON at.tag_id = t.id
            {}
            GROUP BY t.id, t.name
            ORDER BY count DESC
            LIMIT 20
            "#,
            base_where
        );
        
        let mut tag_response = self.db.query_with_params(&tag_query, json!({})).await?;
        let tag_facets: Vec<Value> = tag_response.take(0)?;
        facets.tags = tag_facets.into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect();
        
        // 获取热门作者
        let author_query = format!(
            r#"
            SELECT u.username as value, u.display_name as label, COUNT(DISTINCT a.id) as count
            FROM article a
            JOIN user_profile u ON a.author_id = u.user_id
            {}
            GROUP BY u.user_id, u.username, u.display_name
            ORDER BY count DESC
            LIMIT 20
            "#,
            base_where
        );
        
        let mut author_response = self.db.query_with_params(&author_query, json!({})).await?;
        let author_facets: Vec<Value> = author_response.take(0)?;
        facets.authors = author_facets.into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect();
        
        // 获取出版物
        let publication_query = format!(
            r#"
            SELECT p.slug as value, p.name as label, COUNT(DISTINCT a.id) as count
            FROM article a
            JOIN publication p ON a.publication_id = p.id
            {}
            GROUP BY p.id, p.slug, p.name
            ORDER BY count DESC
            LIMIT 10
            "#,
            base_where
        );
        
        let mut pub_response = self.db.query_with_params(&publication_query, json!({})).await?;
        let pub_facets: Vec<Value> = pub_response.take(0)?;
        facets.publications = pub_facets.into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect();
        
        // 日期范围
        let now = Utc::now();
        facets.date_ranges = vec![
            DateRangeFacet {
                label: "过去24小时".to_string(),
                from: now - Duration::days(1),
                to: now,
                count: self.count_articles_in_date_range(&(now - Duration::days(1)), &now).await?,
            },
            DateRangeFacet {
                label: "过去一周".to_string(),
                from: now - Duration::days(7),
                to: now,
                count: self.count_articles_in_date_range(&(now - Duration::days(7)), &now).await?,
            },
            DateRangeFacet {
                label: "过去一个月".to_string(),
                from: now - Duration::days(30),
                to: now,
                count: self.count_articles_in_date_range(&(now - Duration::days(30)), &now).await?,
            },
            DateRangeFacet {
                label: "过去一年".to_string(),
                from: now - Duration::days(365),
                to: now,
                count: self.count_articles_in_date_range(&(now - Duration::days(365)), &now).await?,
            },
        ];
        
        // 阅读时间范围
        facets.reading_time_ranges = vec![
            RangeFacet {
                label: "快速阅读（< 3分钟）".to_string(),
                min: 0,
                max: 3,
                count: self.count_articles_by_reading_time(0, 3).await?,
            },
            RangeFacet {
                label: "短文（3-5分钟）".to_string(),
                min: 3,
                max: 5,
                count: self.count_articles_by_reading_time(3, 5).await?,
            },
            RangeFacet {
                label: "中等（5-10分钟）".to_string(),
                min: 5,
                max: 10,
                count: self.count_articles_by_reading_time(5, 10).await?,
            },
            RangeFacet {
                label: "长文（> 10分钟）".to_string(),
                min: 10,
                max: 999,
                count: self.count_articles_by_reading_time(10, 999).await?,
            },
        ];
        
        Ok(facets)
    }
    
    /// 计算日期范围内的文章数量
    async fn count_articles_in_date_range(
        &self,
        from: &DateTime<Utc>,
        to: &DateTime<Utc>,
    ) -> Result<i64> {
        let query = r#"
            SELECT count() as count
            FROM article
            WHERE status = 'published'
            AND published_at >= $from
            AND published_at <= $to
        "#;
        
        let mut response = self.db.query_with_params(query, json!({
            "from": from,
            "to": to
        })).await?;
        
        if let Ok(Some(result)) = response.take::<Option<Value>>(0) {
            Ok(result.get("count").and_then(|v| v.as_i64()).unwrap_or(0))
        } else {
            Ok(0)
        }
    }
    
    /// 计算阅读时间范围内的文章数量
    async fn count_articles_by_reading_time(
        &self,
        min: i32,
        max: i32,
    ) -> Result<i64> {
        let query = r#"
            SELECT count() as count
            FROM article
            WHERE status = 'published'
            AND reading_time >= $min
            AND reading_time <= $max
        "#;
        
        let mut response = self.db.query_with_params(query, json!({
            "min": min,
            "max": max
        })).await?;
        
        if let Ok(Some(result)) = response.take::<Option<Value>>(0) {
            Ok(result.get("count").and_then(|v| v.as_i64()).unwrap_or(0))
        } else {
            Ok(0)
        }
    }
}