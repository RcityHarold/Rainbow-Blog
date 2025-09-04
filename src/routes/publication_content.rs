use crate::{
    error::{AppError, Result},
    models::{article::Article, publication::Publication},
    services::auth::User,
    state::AppState,
    utils::middleware::{OptionalAuth, OptionalPublicationContext, RequiredPublicationContext},
};
use axum::{
    extract::{Path, Query, State},
    response::Json,
    routing::{get, post},
    Extension, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, info};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        // Domain-specific content routes - these work with custom domains/subdomains
        .route("/", get(get_publication_home))
        .route("/articles", get(get_publication_articles))
        .route("/articles/:slug", get(get_publication_article))
        .route("/about", get(get_publication_about))
        .route("/writers", get(get_publication_writers))
        // API routes that require publication context
        .route("/api/content/articles", get(api_get_publication_articles))
        .route("/api/content/featured", get(api_get_featured_articles))
}

/// Get publication home page (works with domain routing)
/// GET / (when accessed via custom domain/subdomain)
async fn get_publication_home(
    State(state): State<Arc<AppState>>,
    OptionalAuth(user): OptionalAuth,
    OptionalPublicationContext(pub_context): OptionalPublicationContext,
) -> Result<Json<Value>> {
    info!("Serving publication home page");
    
    match pub_context {
        Some(context) => {
            debug!("Serving home page for publication: {} via domain: {}", 
                   context.publication.name, context.domain);
            
            // Get featured articles for this publication
            let featured_articles = get_featured_articles_for_publication(&state, &context.publication_id).await?;
            
            // Get publication stats
            let stats = get_publication_stats(&state, &context.publication_id).await?;
            
            Ok(Json(json!({
                "type": "publication_home",
                "publication": context.publication,
                "domain": context.domain,
                "is_custom_domain": context.is_custom_domain,
                "featured_articles": featured_articles,
                "stats": stats,
                "user": user.map(|u| json!({
                    "id": u.id,
                    "username": u.username,
                    "email": u.email
                }))
            })))
        }
        None => {
            // Default platform home page
            debug!("Serving default platform home page");
            
            Ok(Json(json!({
                "type": "platform_home",
                "message": "Welcome to Rainbow Blog Platform",
                "user": user.map(|u| json!({
                    "id": u.id,
                    "username": u.username,
                    "email": u.email
                }))
            })))
        }
    }
}

/// Get publication articles (domain-aware)
/// GET /articles (when accessed via custom domain/subdomain)
async fn get_publication_articles(
    State(state): State<Arc<AppState>>,
    OptionalAuth(user): OptionalAuth,
    RequiredPublicationContext(context): RequiredPublicationContext,
    Query(params): Query<ArticleListParams>,
) -> Result<Json<Value>> {
    debug!("Getting articles for publication: {} via domain: {}", 
           context.publication.name, context.domain);
    
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(20);
    let tag = params.tag;
    let search = params.search;
    
    // Get articles for this specific publication
    let articles = state.article_service
        .get_articles_by_publication(&context.publication_id, page, per_page, tag.as_deref(), search.as_deref())
        .await?;
    
    let total_count = state.article_service
        .count_articles_by_publication(&context.publication_id, tag.as_deref(), search.as_deref())
        .await?;
    
    Ok(Json(json!({
        "articles": articles,
        "pagination": {
            "page": page,
            "per_page": per_page,
            "total": total_count,
            "total_pages": (total_count + per_page - 1) / per_page
        },
        "publication": {
            "id": context.publication_id,
            "name": context.publication.name,
            "slug": context.publication.slug
        },
        "domain": context.domain,
        "filters": {
            "tag": tag,
            "search": search
        }
    })))
}

/// Get specific publication article by slug (domain-aware)
/// GET /articles/:slug (when accessed via custom domain/subdomain)
async fn get_publication_article(
    State(state): State<Arc<AppState>>,
    OptionalAuth(user): OptionalAuth,
    RequiredPublicationContext(context): RequiredPublicationContext,
    Path(slug): Path<String>,
) -> Result<Json<Value>> {
    debug!("Getting article '{}' for publication: {} via domain: {}", 
           slug, context.publication.name, context.domain);
    
    // Get article by slug within this publication
    let article = state.article_service
        .get_article_by_slug_in_publication(&slug, &context.publication_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Article not found in this publication".to_string()))?;
    
    // Get related articles from same publication
    let related_articles = state.article_service
        .get_related_articles_in_publication(&article.id, &context.publication_id, 5)
        .await?;
    
    // Increment view count
    if let Err(e) = state.article_service.increment_view_count(&article.id).await {
        tracing::warn!("Failed to increment view count for article {}: {}", article.id, e);
    }
    
    Ok(Json(json!({
        "article": article,
        "related_articles": related_articles,
        "publication": {
            "id": context.publication_id,
            "name": context.publication.name,
            "slug": context.publication.slug
        },
        "domain": context.domain,
        "is_custom_domain": context.is_custom_domain
    })))
}

/// Get publication about page
/// GET /about (when accessed via custom domain/subdomain)
async fn get_publication_about(
    State(state): State<Arc<AppState>>,
    RequiredPublicationContext(context): RequiredPublicationContext,
) -> Result<Json<Value>> {
    debug!("Getting about page for publication: {} via domain: {}", 
           context.publication.name, context.domain);
    
    // Get publication members/writers
    let writers = state.publication_service
        .get_publication_members(&context.publication_id)
        .await?;
    
    // Get publication statistics
    let stats = get_publication_stats(&state, &context.publication_id).await?;
    
    Ok(Json(json!({
        "publication": context.publication,
        "writers": writers,
        "stats": stats,
        "domain": context.domain,
        "is_custom_domain": context.is_custom_domain
    })))
}

/// Get publication writers
/// GET /writers (when accessed via custom domain/subdomain)
async fn get_publication_writers(
    State(state): State<Arc<AppState>>,
    RequiredPublicationContext(context): RequiredPublicationContext,
) -> Result<Json<Value>> {
    debug!("Getting writers for publication: {} via domain: {}", 
           context.publication.name, context.domain);
    
    let members = state.publication_service
        .get_publication_members(&context.publication_id)
        .await?;
    
    // Get article counts for each writer in this publication
    let mut writers_with_stats = Vec::new();
    for member in members {
        let article_count = state.article_service
            .count_articles_by_user_in_publication(&member.user_id, &context.publication_id)
            .await
            .unwrap_or(0);
        
        writers_with_stats.push(json!({
            "member": member,
            "article_count": article_count
        }));
    }
    
    Ok(Json(json!({
        "writers": writers_with_stats,
        "publication": {
            "id": context.publication_id,
            "name": context.publication.name,
            "slug": context.publication.slug
        },
        "domain": context.domain
    })))
}

/// API endpoint to get publication articles (JSON API)
/// GET /api/content/articles (when accessed via custom domain/subdomain)
async fn api_get_publication_articles(
    State(state): State<Arc<AppState>>,
    RequiredPublicationContext(context): RequiredPublicationContext,
    Query(params): Query<ArticleListParams>,
) -> Result<Json<Value>> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(20).min(100); // Max 100 per page
    
    let articles = state.article_service
        .get_articles_by_publication(&context.publication_id, page, per_page, None, None)
        .await?;
    
    let total = state.article_service
        .count_articles_by_publication(&context.publication_id, None, None)
        .await?;
    
    Ok(Json(json!({
        "success": true,
        "data": {
            "articles": articles,
            "pagination": {
                "page": page,
                "per_page": per_page,
                "total": total,
                "has_more": (page * per_page) < total
            }
        },
        "publication_id": context.publication_id
    })))
}

/// API endpoint to get featured articles
/// GET /api/content/featured (when accessed via custom domain/subdomain)
async fn api_get_featured_articles(
    State(state): State<Arc<AppState>>,
    RequiredPublicationContext(context): RequiredPublicationContext,
) -> Result<Json<Value>> {
    let featured_articles = get_featured_articles_for_publication(&state, &context.publication_id).await?;
    
    Ok(Json(json!({
        "success": true,
        "data": {
            "articles": featured_articles
        },
        "publication_id": context.publication_id
    })))
}

// Helper functions

async fn get_featured_articles_for_publication(
    state: &AppState,
    publication_id: &str,
) -> Result<Vec<Value>> {
    // This would get featured articles for the publication
    // For now, returning the latest 5 articles as featured
    let articles = state.article_service
        .get_articles_by_publication(publication_id, 1, 5, None, None)
        .await?;
    
    Ok(articles.into_iter().map(|a| json!(a)).collect())
}

async fn get_publication_stats(
    state: &AppState,
    publication_id: &str,
) -> Result<PublicationStats> {
    let article_count = state.article_service
        .count_articles_by_publication(publication_id, None, None)
        .await?;
    
    let member_count = state.publication_service
        .count_publication_members(publication_id)
        .await
        .unwrap_or(0);
    
    // Get total views for all articles in publication
    let total_views = state.article_service
        .get_total_views_by_publication(publication_id)
        .await
        .unwrap_or(0);
    
    Ok(PublicationStats {
        article_count,
        member_count,
        total_views,
        follower_count: 0, // Implement this when follow system is ready
    })
}

// Data structures

#[derive(Debug, Deserialize)]
struct ArticleListParams {
    page: Option<u64>,
    per_page: Option<u64>,
    tag: Option<String>,
    search: Option<String>,
}

#[derive(Debug, Serialize)]
struct PublicationStats {
    article_count: u64,
    member_count: u64,
    total_views: u64,
    follower_count: u64,
}