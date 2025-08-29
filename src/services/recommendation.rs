use crate::{
    error::{AppError, Result},
    models::{
        recommendation::*,
        article::{Article, ArticleListItem, ArticleStatus, AuthorInfo, PublicationInfo, TagInfo},
        user::UserProfile,
        follow::Follow,
        clap::Clap,
        bookmark::Bookmark,
        comment::Comment,
        tag::Tag,
    },
    services::Database,
};
use std::sync::Arc;
use std::collections::HashMap;
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use tracing::{debug, info, warn};
use uuid::Uuid;

#[derive(Clone)]
pub struct RecommendationService {
    db: Arc<Database>,
}

impl RecommendationService {
    pub async fn new(db: Arc<Database>) -> Result<Self> {
        Ok(Self { db })
    }

    /// 获取用户推荐文章
    pub async fn get_recommendations(
        &self,
        request: RecommendationRequest,
    ) -> Result<RecommendationResult> {
        debug!("Getting recommendations with request: {:?}", request);

        let user_id = request.user_id.as_deref();
        let limit = request.limit.unwrap_or(10);
        let algorithm = request.algorithm.clone().unwrap_or(RecommendationAlgorithm::Hybrid);

        let articles = match algorithm {
            RecommendationAlgorithm::ContentBased => {
                self.content_based_recommendations(user_id, limit, &request).await?
            }
            RecommendationAlgorithm::CollaborativeFiltering => {
                self.collaborative_filtering_recommendations(user_id, limit, &request).await?
            }
            RecommendationAlgorithm::Hybrid => {
                self.hybrid_recommendations(user_id, limit, &request).await?
            }
            RecommendationAlgorithm::Trending => {
                self.trending_recommendations(limit, &request).await?
            }
            RecommendationAlgorithm::Following => {
                self.following_recommendations(user_id, limit, &request).await?
            }
        };

        let total = articles.len();
        Ok(RecommendationResult {
            articles,
            total,
            algorithm_used: format!("{:?}", algorithm),
            generated_at: Utc::now(),
        })
    }

    /// 基于内容的推荐
    async fn content_based_recommendations(
        &self,
        user_id: Option<&str>,
        limit: usize,
        request: &RecommendationRequest,
    ) -> Result<Vec<RecommendedArticle>> {
        debug!("Generating content-based recommendations for user: {:?}", user_id);

        if let Some(uid) = user_id {
            // 获取用户的兴趣标签
            let user_tags = self.get_user_preferred_tags(uid).await?;
            let user_authors = self.get_user_preferred_authors(uid).await?;
            
            // 基于用户兴趣推荐
            self.recommend_by_user_preferences(uid, &user_tags, &user_authors, limit, request).await
        } else {
            // 匿名用户推荐热门内容
            self.trending_recommendations(limit, request).await
        }
    }

    /// 协同过滤推荐
    async fn collaborative_filtering_recommendations(
        &self,
        user_id: Option<&str>,
        limit: usize,
        request: &RecommendationRequest,
    ) -> Result<Vec<RecommendedArticle>> {
        debug!("Generating collaborative filtering recommendations for user: {:?}", user_id);

        if let Some(uid) = user_id {
            // 找到相似用户
            let similar_users = self.find_similar_users(uid).await?;
            
            // 基于相似用户的喜好推荐
            self.recommend_by_similar_users(uid, &similar_users, limit, request).await
        } else {
            // 匿名用户无法使用协同过滤，回退到热门推荐
            self.trending_recommendations(limit, request).await
        }
    }

    /// 混合推荐
    async fn hybrid_recommendations(
        &self,
        user_id: Option<&str>,
        limit: usize,
        request: &RecommendationRequest,
    ) -> Result<Vec<RecommendedArticle>> {
        debug!("Generating hybrid recommendations for user: {:?}", user_id);

        if let Some(uid) = user_id {
            let half_limit = limit / 2;
            
            // 获取内容推荐和协同过滤推荐
            let mut content_recs = self.content_based_recommendations(Some(uid), half_limit, request).await?;
            let mut collab_recs = self.collaborative_filtering_recommendations(Some(uid), half_limit, request).await?;
            
            // 合并和去重
            content_recs.append(&mut collab_recs);
            self.deduplicate_and_rank(content_recs, limit)
        } else {
            self.trending_recommendations(limit, request).await
        }
    }

    /// 热门推荐
    async fn trending_recommendations(
        &self,
        limit: usize,
        request: &RecommendationRequest,
    ) -> Result<Vec<RecommendedArticle>> {
        debug!("Generating trending recommendations");

        let mut query = r#"
            SELECT *, 
                (clap_count * 0.3 + view_count * 0.1 + comment_count * 0.4 + bookmark_count * 0.2) as trending_score
            FROM article 
            WHERE status = 'published' 
            AND is_deleted = false
        "#.to_string();

        let mut params = json!({
            "limit": limit
        });

        // 添加过滤条件
        if let Some(tags) = &request.tags {
            query.push_str(" AND (");
            for (i, tag) in tags.iter().enumerate() {
                if i > 0 { query.push_str(" OR "); }
                query.push_str(&format!("$tag_{} IN tags", i));
                params[format!("tag_{}", i)] = json!(tag);
            }
            query.push_str(")");
        }

        if let Some(authors) = &request.authors {
            query.push_str(" AND author_id IN $authors");
            params["authors"] = json!(authors);
        }

        // 过滤最近7天的文章以获得真正的"热门"
        query.push_str(" AND created_at >= $week_ago");
        params["week_ago"] = json!(Utc::now() - Duration::days(7));

        query.push_str(" ORDER BY trending_score DESC, created_at DESC LIMIT $limit");

        let mut response = self.db.query_with_params(&query, params).await?;
        let articles: Vec<Value> = response.take(0)?;

        let mut recommendations = Vec::new();
        for (i, article_data) in articles.iter().enumerate() {
            if let Ok(article) = serde_json::from_value::<Article>(article_data.clone()) {
                let list_item = self.article_to_list_item(&article).await?;

                let score = article_data.get("trending_score")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);

                recommendations.push(RecommendedArticle {
                    article: list_item,
                    score,
                    reason: "热门文章".to_string(),
                });
            }
        }

        Ok(recommendations)
    }

    /// 关注用户的文章推荐
    async fn following_recommendations(
        &self,
        user_id: Option<&str>,
        limit: usize,
        request: &RecommendationRequest,
    ) -> Result<Vec<RecommendedArticle>> {
        debug!("Generating following recommendations for user: {:?}", user_id);

        let uid = user_id.ok_or_else(|| AppError::Authentication("User ID required for following recommendations".to_string()))?;

        let query = r#"
            SELECT a.*, f.created_at as follow_date
            FROM article a
            JOIN follow f ON a.author_id = f.following_user_id
            WHERE f.follower_user_id = $user_id
            AND a.status = 'published'
            AND a.is_deleted = false
            ORDER BY a.created_at DESC
            LIMIT $limit
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "user_id": uid,
            "limit": limit
        })).await?;

        let articles: Vec<Value> = response.take(0)?;
        let mut recommendations = Vec::new();

        for article_data in articles.iter() {
            if let Ok(article) = serde_json::from_value::<Article>(article_data.clone()) {
                let list_item = self.article_to_list_item(&article).await?;

                recommendations.push(RecommendedArticle {
                    article: list_item,
                    score: 100.0, // 关注的作者给最高分
                    reason: "来自您关注的作者".to_string(),
                });
            }
        }

        Ok(recommendations)
    }

    /// 获取用户偏好标签
    async fn get_user_preferred_tags(&self, user_id: &str) -> Result<Vec<TagPreference>> {
        let query = r#"
            SELECT t.id, t.name, COUNT(*) * 1.0 as weight
            FROM tag t
            JOIN article_tag at ON t.id = at.tag_id
            JOIN article a ON at.article_id = a.id
            JOIN clap c ON a.id = c.article_id
            WHERE c.user_id = $user_id
            GROUP BY t.id, t.name
            ORDER BY weight DESC
            LIMIT 20
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "user_id": user_id
        })).await?;

        let results: Vec<Value> = response.take(0)?;
        let mut preferences = Vec::new();

        for result in results {
            if let (Some(id), Some(name), Some(weight)) = (
                result.get("id").and_then(|v| v.as_str()),
                result.get("name").and_then(|v| v.as_str()),
                result.get("weight").and_then(|v| v.as_f64()),
            ) {
                preferences.push(TagPreference {
                    tag_id: id.to_string(),
                    tag_name: name.to_string(),
                    weight,
                });
            }
        }

        Ok(preferences)
    }

    /// 获取用户偏好作者
    async fn get_user_preferred_authors(&self, user_id: &str) -> Result<Vec<AuthorPreference>> {
        let query = r#"
            SELECT a.author_id, COUNT(*) * 1.0 as weight
            FROM article a
            JOIN clap c ON a.id = c.article_id
            WHERE c.user_id = $user_id
            GROUP BY a.author_id
            ORDER BY weight DESC
            LIMIT 10
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "user_id": user_id
        })).await?;

        let results: Vec<Value> = response.take(0)?;
        let mut preferences = Vec::new();

        for result in results {
            if let (Some(author_id), Some(weight)) = (
                result.get("author_id").and_then(|v| v.as_str()),
                result.get("weight").and_then(|v| v.as_f64()),
            ) {
                preferences.push(AuthorPreference {
                    author_id: author_id.to_string(),
                    weight,
                });
            }
        }

        Ok(preferences)
    }

    /// 基于用户偏好推荐文章
    async fn recommend_by_user_preferences(
        &self,
        user_id: &str,
        tag_preferences: &[TagPreference],
        author_preferences: &[AuthorPreference],
        limit: usize,
        request: &RecommendationRequest,
    ) -> Result<Vec<RecommendedArticle>> {
        let mut recommendations = Vec::new();

        // 基于标签偏好推荐
        if !tag_preferences.is_empty() {
            let tag_recs = self.recommend_by_tags(user_id, tag_preferences, limit / 2).await?;
            recommendations.extend(tag_recs);
        }

        // 基于作者偏好推荐
        if !author_preferences.is_empty() {
            let author_recs = self.recommend_by_authors(user_id, author_preferences, limit / 2).await?;
            recommendations.extend(author_recs);
        }

        // 去重并排序
        Ok(self.deduplicate_and_rank(recommendations, limit)?)
    }

    /// 基于标签推荐
    async fn recommend_by_tags(
        &self,
        user_id: &str,
        tag_preferences: &[TagPreference],
        limit: usize,
    ) -> Result<Vec<RecommendedArticle>> {
        let tag_ids: Vec<&str> = tag_preferences.iter().map(|t| t.tag_id.as_str()).collect();

        let query = r#"
            SELECT DISTINCT a.*
            FROM article a
            JOIN article_tag at ON a.id = at.article_id
            WHERE at.tag_id IN $tag_ids
            AND a.status = 'published'
            AND a.is_deleted = false
            AND a.author_id != $user_id
            AND a.id NOT IN (
                SELECT article_id FROM clap WHERE user_id = $user_id
            )
            ORDER BY a.clap_count DESC, a.created_at DESC
            LIMIT $limit
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "tag_ids": tag_ids,
            "user_id": user_id,
            "limit": limit
        })).await?;

        let articles: Vec<Article> = response.take(0)?;
        let mut recommendations = Vec::new();

        for article in articles {
            let list_item = self.article_to_list_item(&article).await?;

            recommendations.push(RecommendedArticle {
                article: list_item,
                score: 80.0,
                reason: "基于您的兴趣标签".to_string(),
            });
        }

        Ok(recommendations)
    }

    /// 基于作者推荐
    async fn recommend_by_authors(
        &self,
        user_id: &str,
        author_preferences: &[AuthorPreference],
        limit: usize,
    ) -> Result<Vec<RecommendedArticle>> {
        let author_ids: Vec<&str> = author_preferences.iter().map(|a| a.author_id.as_str()).collect();

        let query = r#"
            SELECT * FROM article
            WHERE author_id IN $author_ids
            AND status = 'published'
            AND is_deleted = false
            AND id NOT IN (
                SELECT article_id FROM clap WHERE user_id = $user_id
            )
            ORDER BY created_at DESC
            LIMIT $limit
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "author_ids": author_ids,
            "user_id": user_id,
            "limit": limit
        })).await?;

        let articles: Vec<Article> = response.take(0)?;
        let mut recommendations = Vec::new();

        for article in articles {
            let list_item = self.article_to_list_item(&article).await?;

            recommendations.push(RecommendedArticle {
                article: list_item,
                score: 90.0,
                reason: "来自您喜欢的作者".to_string(),
            });
        }

        Ok(recommendations)
    }

    /// 找到相似用户
    async fn find_similar_users(&self, user_id: &str) -> Result<Vec<String>> {
        // 简化的相似性计算：基于共同点赞的文章
        let query = r#"
            SELECT c2.user_id, COUNT(*) as common_claps
            FROM clap c1
            JOIN clap c2 ON c1.article_id = c2.article_id
            WHERE c1.user_id = $user_id
            AND c2.user_id != $user_id
            GROUP BY c2.user_id
            ORDER BY common_claps DESC
            LIMIT 10
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "user_id": user_id
        })).await?;

        let results: Vec<Value> = response.take(0)?;
        let mut similar_users = Vec::new();

        for result in results {
            if let Some(uid) = result.get("user_id").and_then(|v| v.as_str()) {
                similar_users.push(uid.to_string());
            }
        }

        Ok(similar_users)
    }

    /// 基于相似用户推荐
    async fn recommend_by_similar_users(
        &self,
        user_id: &str,
        similar_users: &[String],
        limit: usize,
        request: &RecommendationRequest,
    ) -> Result<Vec<RecommendedArticle>> {
        if similar_users.is_empty() {
            return Ok(Vec::new());
        }

        let query = r#"
            SELECT DISTINCT a.*, COUNT(*) as popularity
            FROM article a
            JOIN clap c ON a.id = c.article_id
            WHERE c.user_id IN $similar_users
            AND a.status = 'published'
            AND a.is_deleted = false
            AND a.id NOT IN (
                SELECT article_id FROM clap WHERE user_id = $user_id
            )
            GROUP BY a.id
            ORDER BY popularity DESC, a.created_at DESC
            LIMIT $limit
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "similar_users": similar_users,
            "user_id": user_id,
            "limit": limit
        })).await?;

        let articles: Vec<Value> = response.take(0)?;
        let mut recommendations = Vec::new();

        for article_data in articles.iter() {
            if let Ok(article) = serde_json::from_value::<Article>(article_data.clone()) {
                let list_item = self.article_to_list_item(&article).await?;

                let popularity = article_data.get("popularity")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);

                recommendations.push(RecommendedArticle {
                    article: list_item,
                    score: 70.0 + popularity * 5.0,
                    reason: "相似用户喜欢的文章".to_string(),
                });
            }
        }

        Ok(recommendations)
    }

    /// 去重并排序推荐结果
    fn deduplicate_and_rank(
        &self,
        mut recommendations: Vec<RecommendedArticle>,
        limit: usize,
    ) -> Result<Vec<RecommendedArticle>> {
        // 按文章ID去重
        let mut seen = std::collections::HashSet::new();
        recommendations.retain(|rec| seen.insert(rec.article.id.clone()));

        // 按分数排序
        recommendations.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // 限制数量
        recommendations.truncate(limit);

        Ok(recommendations)
    }

    /// 记录用户交互
    pub async fn record_interaction(
        &self,
        user_id: &str,
        article_id: &str,
        interaction_type: InteractionType,
    ) -> Result<()> {
        debug!("Recording interaction: {} -> {} ({:?})", user_id, article_id, interaction_type);

        let interaction = UserInteraction {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            article_id: article_id.to_string(),
            interaction_type: interaction_type.clone(),
            weight: interaction_type.default_weight(),
            created_at: Utc::now(),
        };

        self.db.create("user_interaction", interaction).await?;
        Ok(())
    }

    /// 更新推荐系统缓存
    pub async fn update_recommendations(&self) -> Result<()> {
        info!("Starting recommendation system update");

        // 更新热门文章缓存
        self.update_trending_cache().await?;

        // 计算用户画像
        self.update_user_profiles().await?;

        // 预计算推荐结果（对活跃用户）
        self.precompute_recommendations().await?;

        info!("Recommendation system update completed");
        Ok(())
    }

    /// 更新热门文章缓存
    async fn update_trending_cache(&self) -> Result<()> {
        let query = r#"
            SELECT 
                id as article_id,
                view_count,
                clap_count,
                comment_count,
                bookmark_count,
                created_at,
                (
                    view_count * 0.1 + 
                    clap_count * 0.3 + 
                    comment_count * 0.4 + 
                    bookmark_count * 0.2 +
                    (CASE WHEN created_at > $week_ago THEN 20 ELSE 0 END)
                ) as trending_score
            FROM article
            WHERE status = 'published' 
            AND is_deleted = false
            ORDER BY trending_score DESC
        "#;

        let week_ago = Utc::now() - Duration::days(7);
        let mut response = self.db.query_with_params(query, json!({
            "week_ago": week_ago
        })).await?;

        let trending_metrics: Vec<Value> = response.take(0)?;

        // 清理旧的趋势数据
        self.db.query_with_params("DELETE trending_metrics WHERE calculated_at < $yesterday", json!({
            "yesterday": Utc::now() - Duration::days(1)
        })).await?;

        // 插入新的趋势数据
        for metric in trending_metrics {
            if let Some(article_id) = metric.get("article_id").and_then(|v| v.as_str()) {
                let trending_metric = TrendingMetrics {
                    article_id: article_id.to_string(),
                    views_24h: 0, // 简化版本，可以后续优化
                    views_7d: metric.get("view_count").and_then(|v| v.as_i64()).unwrap_or(0),
                    claps_24h: 0,
                    claps_7d: metric.get("clap_count").and_then(|v| v.as_i64()).unwrap_or(0),
                    comments_24h: 0,
                    comments_7d: metric.get("comment_count").and_then(|v| v.as_i64()).unwrap_or(0),
                    trending_score: metric.get("trending_score").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    calculated_at: Utc::now(),
                };

                self.db.create("trending_metrics", trending_metric).await?;
            }
        }

        Ok(())
    }

    /// 更新用户画像
    async fn update_user_profiles(&self) -> Result<()> {
        // 获取活跃用户列表
        let query = r#"
            SELECT DISTINCT user_id
            FROM user_interaction
            WHERE created_at > $week_ago
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "week_ago": Utc::now() - Duration::days(7)
        })).await?;

        let user_ids: Vec<Value> = response.take(0)?;

        for user_data in user_ids {
            if let Some(user_id) = user_data.get("user_id").and_then(|v| v.as_str()) {
                let _ = self.build_user_profile(user_id).await; // 忽略单个用户的错误
            }
        }

        Ok(())
    }

    /// 构建用户画像
    async fn build_user_profile(&self, user_id: &str) -> Result<()> {
        let tag_preferences = self.get_user_preferred_tags(user_id).await?;
        let author_preferences = self.get_user_preferred_authors(user_id).await?;

        // 计算平均阅读时间
        let avg_reading_time_query = r#"
            SELECT AVG(a.reading_time) as avg_time
            FROM article a
            JOIN user_interaction ui ON a.id = ui.article_id
            WHERE ui.user_id = $user_id
            AND ui.interaction_type = 'ReadComplete'
        "#;

        let mut response = self.db.query_with_params(avg_reading_time_query, json!({
            "user_id": user_id
        })).await?;

        let avg_time_result: Vec<Value> = response.take(0)?;
        let avg_reading_time = avg_time_result.first()
            .and_then(|v| v.get("avg_time"))
            .and_then(|v| v.as_f64())
            .unwrap_or(5.0);

        // 计算总交互数
        let total_interactions_query = r#"
            SELECT COUNT(*) as total
            FROM user_interaction
            WHERE user_id = $user_id
        "#;

        let mut response = self.db.query_with_params(total_interactions_query, json!({
            "user_id": user_id
        })).await?;

        let total_result: Vec<Value> = response.take(0)?;
        let total_interactions = total_result.first()
            .and_then(|v| v.get("total"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let profile = crate::models::recommendation::UserProfile {
            user_id: user_id.to_string(),
            preferred_tags: tag_preferences,
            preferred_authors: author_preferences,
            avg_reading_time,
            total_interactions,
            last_updated: Utc::now(),
        };

        // 删除旧的用户画像
        let delete_query = "DELETE user_profile_recommendation WHERE user_id = $user_id";
        self.db.query_with_params(delete_query, json!({
            "user_id": user_id
        })).await?;

        // 创建新的用户画像
        self.db.create("user_profile_recommendation", profile).await?;

        Ok(())
    }

    /// 预计算推荐结果
    async fn precompute_recommendations(&self) -> Result<()> {
        // 简化版本：只为最活跃的用户预计算
        let active_users_query = r#"
            SELECT user_id, COUNT(*) as interaction_count
            FROM user_interaction
            WHERE created_at > $week_ago
            GROUP BY user_id
            ORDER BY interaction_count DESC
            LIMIT 100
        "#;

        let mut response = self.db.query_with_params(active_users_query, json!({
            "week_ago": Utc::now() - Duration::days(7)
        })).await?;

        let active_users: Vec<Value> = response.take(0)?;

        for user_data in active_users {
            if let Some(user_id) = user_data.get("user_id").and_then(|v| v.as_str()) {
                let request = RecommendationRequest {
                    user_id: Some(user_id.to_string()),
                    limit: Some(20),
                    exclude_read: Some(true),
                    algorithm: Some(RecommendationAlgorithm::Hybrid),
                    tags: None,
                    authors: None,
                };

                // 预计算并缓存推荐结果
                if let Ok(recommendations) = self.get_recommendations(request).await {
                    // 这里可以将结果存储到缓存表中
                    debug!("Precomputed {} recommendations for user {}", 
                          recommendations.articles.len(), user_id);
                }
            }
        }

        Ok(())
    }

    /// 获取相关文章推荐
    pub async fn get_related_articles(
        &self,
        article_id: &str,
        limit: usize,
    ) -> Result<Vec<RecommendedArticle>> {
        debug!("Getting related articles for article: {}", article_id);

        // 获取目标文章信息
        let article: Article = self.db.get_by_id("article", article_id).await?
            .ok_or_else(|| AppError::NotFound("Article not found".to_string()))?;

        // 基于标签找相关文章
        let query = r#"
            SELECT DISTINCT a.*, COUNT(at1.tag_id) as common_tags
            FROM article a
            JOIN article_tag at1 ON a.id = at1.article_id
            JOIN article_tag at2 ON at1.tag_id = at2.tag_id
            WHERE at2.article_id = $article_id
            AND a.id != $article_id
            AND a.status = 'published'
            AND a.is_deleted = false
            GROUP BY a.id
            ORDER BY common_tags DESC, a.clap_count DESC
            LIMIT $limit
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "article_id": article_id,
            "limit": limit
        })).await?;

        let articles: Vec<Value> = response.take(0)?;
        let mut recommendations = Vec::new();

        for article_data in articles.iter() {
            if let Ok(related_article) = serde_json::from_value::<Article>(article_data.clone()) {
                let list_item = self.article_to_list_item(&related_article).await?;

                let common_tags = article_data.get("common_tags")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);

                recommendations.push(RecommendedArticle {
                    article: list_item,
                    score: common_tags * 10.0 + related_article.clap_count as f64 * 0.1,
                    reason: "相关主题".to_string(),
                });
            }
        }

        Ok(recommendations)
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
        
        // Get tags info
        let tags_query = r#"
            SELECT t.id, t.name, t.slug
            FROM tag t
            JOIN article_tag at ON t.id = at.tag_id
            WHERE at.article_id = $article_id
        "#;
        
        let mut tags_response = self.db.query_with_params(tags_query, json!({
            "article_id": &article.id
        })).await?;
        
        let tags_data: Vec<Value> = tags_response.take(0)?;
        let tags: Vec<TagInfo> = tags_data.into_iter().map(|t| TagInfo {
            id: t["id"].as_str().unwrap_or("").to_string(),
            name: t["name"].as_str().unwrap_or("").to_string(),
            slug: t["slug"].as_str().unwrap_or("").to_string(),
        }).collect();
        
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