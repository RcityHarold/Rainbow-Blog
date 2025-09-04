#[cfg(test)]
mod domain_routing_tests {
    use super::*;
    
    // Note: These are example tests showing what should be tested
    // Actual tests would require setting up a test database and mock services
    
    #[tokio::test]
    async fn test_domain_extraction() {
        // Test that the middleware correctly extracts domains from Host header
        
        // Test cases:
        // - "platform.com" -> "platform.com"
        // - "platform.com:3000" -> "platform.com" (port removed)
        // - "myblog.platform.com" -> "myblog.platform.com"
        // - "blog.example.com:8080" -> "blog.example.com"
    }
    
    #[tokio::test]
    async fn test_subdomain_resolution() {
        // Test subdomain-to-publication resolution
        
        // Given: subdomain "myblog.platform.com" exists and maps to publication "pub_123"
        // When: middleware processes request with Host: "myblog.platform.com"
        // Then: publication context should be created with pub_123
    }
    
    #[tokio::test]
    async fn test_custom_domain_resolution() {
        // Test custom domain-to-publication resolution
        
        // Given: custom domain "blog.example.com" exists and maps to publication "pub_456"
        // When: middleware processes request with Host: "blog.example.com"
        // Then: publication context should be created with pub_456 and is_custom_domain=true
    }
    
    #[tokio::test]
    async fn test_unknown_domain_handling() {
        // Test handling of domains not mapped to any publication
        
        // Given: domain "unknown.example.com" doesn't exist in database
        // When: middleware processes request with Host: "unknown.example.com"
        // Then: request should proceed without publication context
    }
    
    #[tokio::test]
    async fn test_publication_context_extraction() {
        // Test that handlers can extract publication context correctly
        
        // Test OptionalPublicationContext:
        // - Returns Some(context) when publication context exists
        // - Returns None when no publication context
        
        // Test RequiredPublicationContext:
        // - Returns context when publication context exists
        // - Returns 400 Bad Request when no publication context
    }
    
    #[tokio::test]
    async fn test_domain_specific_routing() {
        // Test that routes behave differently based on domain
        
        // Test cases:
        // - GET / via "platform.com" -> platform homepage
        // - GET / via "myblog.platform.com" -> publication homepage
        // - GET /articles via "myblog.platform.com" -> publication articles only
    }
    
    #[tokio::test]
    async fn test_api_route_consistency() {
        // Test that API routes work consistently across domains
        
        // Test cases:
        // - GET /api/blog/articles via any domain -> full API functionality
        // - Publication context available but doesn't change API behavior
    }
    
    #[tokio::test]
    async fn test_domain_specific_api_routes() {
        // Test domain-specific API endpoints
        
        // Test cases:
        // - GET /api/content/articles via publication domain -> filtered results
        // - GET /api/content/articles via platform domain -> error or all results
    }
    
    #[tokio::test]
    async fn test_ssl_detection() {
        // Test HTTPS detection for SSL-enabled domains
        
        // Test cases:
        // - Request with X-Forwarded-Proto: https
        // - Request with X-Forwarded-SSL: on
        // - Direct HTTPS request
    }
    
    #[tokio::test]
    async fn test_middleware_error_handling() {
        // Test middleware behavior when services are unavailable
        
        // Test cases:
        // - Domain service returns error -> request proceeds without context
        // - Publication service returns error -> request proceeds without context
        // - Database is unavailable -> request proceeds without context
    }
    
    #[tokio::test]
    async fn test_performance_with_caching() {
        // Test that domain resolution is cached for performance
        
        // Test that:
        // - First request queries database
        // - Subsequent requests use cached results
        // - Cache invalidation works correctly
    }
}

// Example test setup (would need actual implementation)
/*
use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use tower::ServiceExt;

async fn create_test_app() -> Router {
    // Create test app with domain routing middleware
    Router::new()
        .route("/", axum::routing::get(test_handler))
        .layer(axum::middleware::from_fn_with_state(
            test_app_state(),
            domain_routing_middleware,
        ))
}

async fn test_handler(
    OptionalPublicationContext(context): OptionalPublicationContext,
) -> &'static str {
    match context {
        Some(_) => "publication_context",
        None => "no_context",
    }
}

fn test_app_state() -> Arc<AppState> {
    // Create mock app state for testing
    todo!("Implement test app state")
}

#[tokio::test]
async fn integration_test_example() {
    let app = create_test_app().await;
    
    // Test request to main platform
    let request = Request::builder()
        .uri("/")
        .header("host", "platform.com")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert_eq!(body_str, "no_context");
    
    // Test request to publication subdomain
    let request = Request::builder()
        .uri("/")
        .header("host", "myblog.platform.com")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert_eq!(body_str, "publication_context");
}
*/