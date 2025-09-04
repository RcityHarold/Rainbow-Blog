# Domain Routing Middleware

This middleware handles incoming requests based on domain/subdomain routing, enabling multi-tenant functionality where different domains can serve different publications.

## Overview

The domain routing middleware:

1. **Extracts the Host Header**: Gets the domain from the incoming request
2. **Resolves Domain to Publication**: Uses the domain service to find which publication owns the domain
3. **Adds Publication Context**: Injects publication information into the request for downstream handlers
4. **Enables Domain-Specific Routing**: Allows the same routes to serve different content based on the domain

## How It Works

### Middleware Flow

```
Incoming Request with Host Header
         ↓
Extract and clean domain (remove port)
         ↓
Query domain service to find publication
         ↓
If publication found:
  - Get publication details
  - Create PublicationContext
  - Add to request extensions
         ↓
Continue to next middleware/handler
```

### Domain Types Supported

1. **Subdomains**: `myblog.platform.com`
2. **Custom Domains**: `blog.example.com`

### Publication Context Structure

```rust
pub struct PublicationContext {
    pub publication_id: String,
    pub publication: Publication,
    pub domain: String,
    pub is_custom_domain: bool,
}
```

## Configuration

The middleware requires these environment variables:

```bash
# Base domain for subdomains (e.g., "platform.com")
BASE_DOMAIN=platform.local

# SSL configuration (optional)
SSL_PROVIDER_ENDPOINT=https://ssl-provider.com/api
SSL_PROVIDER_API_KEY=your_api_key
AUTO_PROVISION_SSL=true
SSL_WEBHOOK_URL=https://your-app.com/ssl-webhook
```

## Usage Examples

### Setting Up Domain Routing

The middleware is automatically applied in `main.rs`:

```rust
let app = Router::new()
    // Your routes here
    .layer(middleware::from_fn_with_state(
        app_state.clone(),
        utils::middleware::domain_routing_middleware,
    ))
    .with_state(app_state);
```

### Using Publication Context in Handlers

#### Optional Publication Context

```rust
use crate::utils::middleware::OptionalPublicationContext;

async fn handler(
    OptionalPublicationContext(context): OptionalPublicationContext,
) -> Result<Json<Value>> {
    match context {
        Some(pub_context) => {
            // Handle request with publication context
            println!("Serving {} via {}", pub_context.publication.name, pub_context.domain);
            // ... publication-specific logic
        }
        None => {
            // Handle request without publication context (main platform)
            // ... default platform logic
        }
    }
}
```

#### Required Publication Context

```rust
use crate::utils::middleware::RequiredPublicationContext;

async fn publication_only_handler(
    RequiredPublicationContext(context): RequiredPublicationContext,
) -> Result<Json<Value>> {
    // This handler only works when accessed via a publication domain
    // Returns error if no publication context is available
    
    let articles = get_articles_for_publication(&context.publication_id).await?;
    
    Ok(Json(json!({
        "publication": context.publication.name,
        "domain": context.domain,
        "articles": articles
    })))
}
```

## Domain-Specific Routes

### Publication Content Routes

These routes work differently based on the domain:

```rust
// GET / 
// - Via platform.com → Platform homepage
// - Via myblog.platform.com → MyBlog homepage
// - Via blog.example.com → Custom domain blog homepage

// GET /articles
// - Via platform.com → All platform articles
// - Via myblog.platform.com → MyBlog articles only
// - Via blog.example.com → Custom domain blog articles only
```

### API Routes

API routes maintain the `/api/blog/` prefix and work regardless of domain:

```rust
// These work the same on any domain:
// - platform.com/api/blog/articles
// - myblog.platform.com/api/blog/articles
// - blog.example.com/api/blog/articles
```

### Domain-Specific API Routes

Some API routes can also be domain-aware:

```rust
// GET /api/content/articles
// - Returns articles filtered by publication context if available
// - Returns all articles if no publication context
```

## Domain Management

### Creating Subdomains

```bash
POST /api/blog/publications/{id}/domains/subdomain
{
    "subdomain": "myblog",
    "is_primary": true
}
```

### Adding Custom Domains

```bash
POST /api/blog/publications/{id}/domains/custom
{
    "domain": "blog.example.com",
    "is_primary": false
}
```

### Domain Verification

Custom domains require DNS verification:

1. Add TXT record: `_rainbow-verify.blog.example.com` → `rainbow-verify-token`
2. Add CNAME record: `blog.example.com` → `domains.platform.com`
3. Verify: `POST /api/blog/domains/{domain_id}/verify`

## Error Handling

### Domain Not Found

When a domain doesn't map to any publication:
- Request proceeds without publication context
- `OptionalPublicationContext` will be `None`
- `RequiredPublicationContext` will return 400 Bad Request

### Publication Not Found

When domain maps to a publication ID but publication doesn't exist:
- Logs warning
- Request proceeds without publication context

### DNS Resolution Errors

- Logged as warnings
- Request proceeds without failing
- Domain verification endpoints will report specific errors

## Security Considerations

### Domain Validation

- Subdomains are validated for format and availability
- Custom domains are validated for format and ownership
- DNS verification required for custom domains

### SSL/TLS

- Automatic SSL provisioning for verified domains
- SSL status tracking and renewal
- HSTS headers for HTTPS domains

### Rate Limiting

- Rate limiting applies per IP regardless of domain
- Could be extended to rate limit per publication

## Performance

### Caching

- Domain-to-publication mapping should be cached
- Publication details should be cached
- DNS resolution should be cached

### Database Queries

- One query to resolve domain to publication ID
- One query to get publication details (cached)
- Queries only happen when publication context is needed

## Monitoring

### Logging

- Domain resolution attempts
- Publication context creation
- SSL provisioning events
- DNS verification results

### Metrics

- Domain resolution success/failure rates
- SSL certificate status
- Publication access patterns

## Migration and Compatibility

### Existing Routes

All existing routes continue to work via the main platform domain:
- `platform.com/api/blog/*` → Full API access
- `platform.com/` → Platform homepage

### New Domain Routes

Domain-specific routes are additive:
- `myblog.platform.com/` → Publication homepage
- `myblog.platform.com/articles` → Publication articles

### Backwards Compatibility

- No breaking changes to existing API
- Publication context is optional for most routes
- Existing clients work without modification

## Troubleshooting

### Common Issues

1. **Domain not resolving**
   - Check DNS records
   - Verify domain is added to publication
   - Check domain verification status

2. **Publication context not available**
   - Verify middleware is applied
   - Check domain service configuration
   - Ensure publication exists and is active

3. **SSL issues**
   - Check SSL provider configuration
   - Verify domain verification is complete
   - Check SSL certificate status

### Debug Logging

Enable debug logging to see middleware operation:

```bash
LOG_LEVEL=rainbow_blog=debug,tower_http=debug
```

This will show:
- Domain extraction from Host header
- Domain-to-publication resolution
- Publication context creation
- SSL provisioning attempts