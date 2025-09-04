// Example of how the domain routing middleware works
//
// Run with: cargo run --example domain_routing_example

use serde_json::json;

/// Example scenarios demonstrating domain routing middleware functionality
fn main() {
    println!("=== Domain Routing Middleware Examples ===\n");

    // Scenario 1: Main platform access
    println!("1. Request to main platform:");
    println!("   URL: https://platform.com/");
    println!("   Host: platform.com");
    println!("   Result: No publication context, serves platform homepage");
    println!("   Response: Platform welcome page with all publications\n");

    // Scenario 2: Subdomain access
    println!("2. Request to subdomain:");
    println!("   URL: https://myblog.platform.com/");
    println!("   Host: myblog.platform.com");
    println!("   Middleware Actions:");
    println!("     - Extracts 'myblog.platform.com' from Host header");
    println!("     - Queries domain service: find_publication_by_domain('myblog.platform.com')");
    println!("     - Finds publication ID: 'pub_123'");
    println!("     - Gets publication details");
    println!("     - Creates PublicationContext");
    println!("     - Adds to request extensions");
    println!("   Result: Publication-specific homepage served");
    println!("   Context: {{");
    println!("     publication_id: 'pub_123',");
    println!("     domain: 'myblog.platform.com',");
    println!("     is_custom_domain: false");
    println!("   }}\n");

    // Scenario 3: Custom domain access
    println!("3. Request to custom domain:");
    println!("   URL: https://blog.example.com/articles");
    println!("   Host: blog.example.com");
    println!("   Middleware Actions:");
    println!("     - Extracts 'blog.example.com' from Host header");
    println!("     - Queries domain service: find_publication_by_domain('blog.example.com')");
    println!("     - Finds publication ID: 'pub_456'");
    println!("     - Gets publication details");
    println!("     - Creates PublicationContext");
    println!("     - Adds to request extensions");
    println!("   Result: Publication articles served (filtered by publication)");
    println!("   Context: {{");
    println!("     publication_id: 'pub_456',");
    println!("     domain: 'blog.example.com',");
    println!("     is_custom_domain: true");
    println!("   }}\n");

    // Scenario 4: API access (domain-agnostic)
    println!("4. API request through any domain:");
    println!("   URL: https://myblog.platform.com/api/blog/articles");
    println!("   Host: myblog.platform.com");
    println!("   Result: Full API access regardless of domain");
    println!("   Note: Publication context still available for filtering if needed\n");

    // Scenario 5: Domain-specific API
    println!("5. Domain-specific API request:");
    println!("   URL: https://myblog.platform.com/api/content/articles");
    println!("   Host: myblog.platform.com");
    println!("   Result: Articles filtered by publication context");
    println!("   Handler uses RequiredPublicationContext extractor\n");

    // Scenario 6: Unknown domain
    println!("6. Request to unmapped domain:");
    println!("   URL: https://unknown.example.com/");
    println!("   Host: unknown.example.com");
    println!("   Middleware Actions:");
    println!("     - Extracts 'unknown.example.com' from Host header");
    println!("     - Queries domain service: find_publication_by_domain('unknown.example.com')");
    println!("     - Returns None (no publication found)");
    println!("     - No publication context added");
    println!("   Result: Request proceeds without publication context");
    println!("   Handler behavior:");
    println!("     - OptionalPublicationContext: returns None");
    println!("     - RequiredPublicationContext: returns 400 Bad Request\n");

    println!("=== Handler Examples ===\n");

    // Handler examples
    show_handler_examples();

    println!("=== Configuration Examples ===\n");
    
    show_configuration_examples();

    println!("=== Use Cases ===\n");
    
    show_use_cases();
}

fn show_handler_examples() {
    println!("Handler using OptionalPublicationContext:");
    println!(r#"
async fn homepage(
    OptionalPublicationContext(context): OptionalPublicationContext,
) -> Result<Json<Value>> {{
    match context {{
        Some(pub_context) => {{
            // Domain-specific homepage
            Ok(Json(json!({{
                "type": "publication_homepage",
                "publication": pub_context.publication.name,
                "domain": pub_context.domain,
                "articles": get_publication_articles(&pub_context.publication_id).await?
            }})))
        }}
        None => {{
            // Platform homepage
            Ok(Json(json!({{
                "type": "platform_homepage",
                "message": "Welcome to Rainbow Blog Platform",
                "featured_publications": get_featured_publications().await?
            }})))
        }}
    }}
}}
"#);

    println!("Handler using RequiredPublicationContext:");
    println!(r#"
async fn publication_articles(
    RequiredPublicationContext(context): RequiredPublicationContext,
) -> Result<Json<Value>> {{
    // This only works when accessed via publication domain
    let articles = get_articles_by_publication(&context.publication_id).await?;
    
    Ok(Json(json!({{
        "publication": context.publication.name,
        "domain": context.domain,
        "articles": articles
    }})))
}}
"#);
}

fn show_configuration_examples() {
    println!("Environment Configuration:");
    println!("BASE_DOMAIN=platform.com");
    println!("SSL_PROVIDER_ENDPOINT=https://ssl-provider.com/api");
    println!("SSL_PROVIDER_API_KEY=your_api_key");
    println!("AUTO_PROVISION_SSL=true\n");

    println!("Domain Service Configuration:");
    println!(r#"
let domain_config = DomainConfig {{
    base_domain: "platform.com".to_string(),
    dns_verification_timeout: 300,
    ssl_provider_endpoint: Some("https://ssl-provider.com/api".to_string()),
    ssl_provider_api_key: Some("your_api_key".to_string()),
    auto_provision_ssl: true,
    ssl_webhook_url: Some("https://platform.com/ssl-webhook".to_string()),
}};
"#);
}

fn show_use_cases() {
    println!("1. Multi-tenant Blog Platform:");
    println!("   - Each publication gets its own subdomain");
    println!("   - Custom domains supported for premium users");
    println!("   - Content filtered automatically by domain");
    println!();
    
    println!("2. Corporate Blog Networks:");
    println!("   - Different departments get subdomains");
    println!("   - External blogs use custom domains");
    println!("   - Unified management through main platform");
    println!();
    
    println!("3. White-label Solutions:");
    println!("   - Customers use their own domains");
    println!("   - Platform branding hidden on custom domains");
    println!("   - SSL certificates automatically provisioned");
    println!();
    
    println!("4. Content Hub:");
    println!("   - Different content categories get subdomains");
    println!("   - Cross-publication content discovery");
    println!("   - Centralized user management");
}