use crate::{
    error::{AppError, Result},
    models::domain::*,
    services::auth::User,
    state::AppState,
    utils::middleware::OptionalAuth,
};
use axum::{
    extract::{Path, Query, State},
    response::Json,
    routing::{delete, get, post, put},
    Extension, Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::debug;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        // Publication domain routes
        .route("/publications/:id/domains/subdomain", post(create_subdomain))
        .route("/publications/:id/domains/custom", post(add_custom_domain))
        .route("/publications/:id/domains", get(list_publication_domains))
        // Domain-specific routes
        .route("/domains/:domain_id", get(get_domain_details).put(update_domain).delete(delete_domain))
        .route("/domains/:domain_id/verify", post(verify_domain))
        .route("/domains/check-availability", post(check_domain_availability))
        .route("/domains/resolve/:domain", get(resolve_domain))
}

/// Create subdomain for publication
/// POST /api/publications/:id/domains/subdomain
async fn create_subdomain(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(publication_id): Path<String>,
    Json(request): Json<CreateSubdomainRequest>,
) -> Result<Json<Value>> {
    debug!("Creating subdomain for publication: {} by user: {}", publication_id, user.id);

    // Validate the subdomain request
    if let Err(errors) = request.validate() {
        return Err(AppError::Validation(errors.join(", ")));
    }

    // Check if user has permission to manage domains for this publication
    let has_permission = check_publication_permission(&state, &publication_id, &user.id).await?;
    if !has_permission {
        return Err(AppError::Authorization(
            "You don't have permission to manage domains for this publication".to_string()
        ));
    }

    // Create the subdomain
    let domain_response = state
        .domain_service
        .create_subdomain(&publication_id, request)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": domain_response,
        "message": "Subdomain created successfully"
    })))
}

/// Add custom domain to publication
/// POST /api/publications/:id/domains/custom
async fn add_custom_domain(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(publication_id): Path<String>,
    Json(request): Json<AddCustomDomainRequest>,
) -> Result<Json<Value>> {
    debug!("Adding custom domain for publication: {} by user: {}", publication_id, user.id);

    // Validate the custom domain request
    if let Err(errors) = request.validate() {
        return Err(AppError::Validation(errors.join(", ")));
    }

    // Check if user has permission to manage domains for this publication
    let has_permission = check_publication_permission(&state, &publication_id, &user.id).await?;
    if !has_permission {
        return Err(AppError::Authorization(
            "You don't have permission to manage domains for this publication".to_string()
        ));
    }

    // Add the custom domain
    let domain_response = state
        .domain_service
        .add_custom_domain(&publication_id, request)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": domain_response,
        "message": "Custom domain added successfully. Please configure DNS records for verification."
    })))
}

/// List domains for a publication
/// GET /api/publications/:id/domains
async fn list_publication_domains(
    State(state): State<Arc<AppState>>,
    Path(publication_id): Path<String>,
    OptionalAuth(user): OptionalAuth,
) -> Result<Json<Value>> {
    debug!("Listing domains for publication: {}", publication_id);

    // Get domains for the publication
    let domains = state
        .domain_service
        .get_publication_domains(&publication_id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": domains
    })))
}

/// Get domain details
/// GET /api/domains/:domain_id
async fn get_domain_details(
    State(state): State<Arc<AppState>>,
    Path(domain_id): Path<String>,
    Extension(user): Extension<User>,
) -> Result<Json<Value>> {
    debug!("Getting domain details: {} for user: {}", domain_id, user.id);

    // Get the domain
    let domain = state
        .domain_service
        .get_domain(&domain_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Domain not found".to_string()))?;

    // Check if user has permission to view this domain
    let has_permission = check_publication_permission(&state, &domain.domain.publication_id.to_string(), &user.id).await?;
    if !has_permission {
        return Err(AppError::Authorization(
            "You don't have permission to view this domain".to_string()
        ));
    }

    Ok(Json(json!({
        "success": true,
        "data": domain
    })))
}

/// Trigger domain verification
/// POST /api/domains/:domain_id/verify
async fn verify_domain(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(domain_id): Path<String>,
) -> Result<Json<Value>> {
    debug!("Verifying domain: {} by user: {}", domain_id, user.id);

    // Get the domain to check permissions
    let domain = state
        .domain_service
        .get_domain(&domain_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Domain not found".to_string()))?;

    // Check if user has permission to manage this domain
    let has_permission = check_publication_permission(&state, &domain.domain.publication_id.to_string(), &user.id).await?;
    if !has_permission {
        return Err(AppError::Authorization(
            "You don't have permission to verify this domain".to_string()
        ));
    }

    // Trigger verification
    let verification_response = state
        .domain_service
        .verify_domain(&domain_id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": verification_response,
        "message": if verification_response.verified {
            "Domain verified successfully"
        } else {
            "Domain verification in progress. Please check DNS records."
        }
    })))
}

/// Delete domain
/// DELETE /api/domains/:domain_id
async fn delete_domain(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(domain_id): Path<String>,
) -> Result<Json<Value>> {
    debug!("Deleting domain: {} by user: {}", domain_id, user.id);

    // Get the domain to check permissions
    let domain = state
        .domain_service
        .get_domain(&domain_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Domain not found".to_string()))?;

    // Check if user has permission to manage this domain
    let has_permission = check_publication_permission(&state, &domain.domain.publication_id.to_string(), &user.id).await?;
    if !has_permission {
        return Err(AppError::Authorization(
            "You don't have permission to delete this domain".to_string()
        ));
    }

    // Delete the domain
    state
        .domain_service
        .delete_domain(&domain_id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "Domain deleted successfully"
    })))
}

/// Update domain settings
/// PUT /api/domains/:domain_id
async fn update_domain(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(domain_id): Path<String>,
    Json(request): Json<UpdateDomainRequest>,
) -> Result<Json<Value>> {
    debug!("Updating domain: {} by user: {}", domain_id, user.id);

    // Get the domain to check permissions
    let domain = state
        .domain_service
        .get_domain(&domain_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Domain not found".to_string()))?;

    // Check if user has permission to manage this domain
    let has_permission = check_publication_permission(&state, &domain.domain.publication_id.to_string(), &user.id).await?;
    if !has_permission {
        return Err(AppError::Authorization(
            "You don't have permission to update this domain".to_string()
        ));
    }

    // Update the domain
    let updated_domain = state
        .domain_service
        .update_domain(&domain_id, request)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": updated_domain,
        "message": "Domain updated successfully"
    })))
}

/// Check domain availability
/// POST /api/domains/check-availability
async fn check_domain_availability(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Json(request): Json<CheckDomainAvailabilityRequest>,
) -> Result<Json<Value>> {
    debug!("Checking domain availability: {} for user: {}", request.domain, user.id);

    // Check if the domain is available
    let availability = check_domain_available(&state, &request.domain, request.domain_type).await?;

    Ok(Json(json!({
        "success": true,
        "data": availability
    })))
}

/// Resolve domain to publication
/// GET /api/domains/resolve/:domain
async fn resolve_domain(
    State(state): State<Arc<AppState>>,
    Path(domain): Path<String>,
) -> Result<Json<Value>> {
    debug!("Resolving domain: {}", domain);

    // Find publication by domain
    let publication_id = state
        .domain_service
        .find_publication_by_domain(&domain)
        .await?;

    match publication_id {
        Some(pub_id) => Ok(Json(json!({
            "success": true,
            "data": {
                "publication_id": pub_id,
                "domain": domain
            }
        }))),
        None => Err(AppError::NotFound("No publication found for this domain".to_string()))
    }
}

/// Helper function to check if user has permission to manage domains for a publication
async fn check_publication_permission(
    state: &Arc<AppState>,
    publication_id: &str,
    user_id: &str,
) -> Result<bool> {
    // Get publication to check ownership
    let publication = state
        .publication_service
        .get_publication(publication_id, Some(user_id))
        .await?
        .ok_or_else(|| AppError::NotFound("Publication not found".to_string()))?;

    // Check if user is owner or editor
    if publication.publication.owner_id == user_id {
        return Ok(true);
    }

    // Check if user is an editor
    if let Some(member) = publication.member {
        if member.role == "editor" || member.role == "admin" {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Helper function to check domain availability
async fn check_domain_available(
    state: &Arc<AppState>,
    domain: &str,
    domain_type: DomainType,
) -> Result<DomainAvailabilityResponse> {
    // For subdomains, check if it's already taken
    if domain_type == DomainType::Subdomain {
        // Assume a default base domain - in production, this should come from config
        let base_domain = "platform.com"; // TODO: Get from config
        let full_domain = format!("{}.{}", domain, base_domain);
        
        // Check if subdomain already exists
        let existing = state
            .domain_service
            .find_publication_by_domain(&full_domain)
            .await?;

        if existing.is_some() {
            return Ok(DomainAvailabilityResponse {
                available: false,
                domain: domain.to_string(),
                domain_type,
                reason: Some("Subdomain is already taken".to_string()),
            });
        }

        // Check reserved subdomains
        let reserved = vec!["www", "api", "admin", "app", "blog", "mail", "ftp", "ssh"];
        if reserved.contains(&domain.to_lowercase().as_str()) {
            return Ok(DomainAvailabilityResponse {
                available: false,
                domain: domain.to_string(),
                domain_type,
                reason: Some("This subdomain is reserved".to_string()),
            });
        }
    } else {
        // For custom domains, check if it's already registered
        let existing = state
            .domain_service
            .find_publication_by_domain(domain)
            .await?;

        if existing.is_some() {
            return Ok(DomainAvailabilityResponse {
                available: false,
                domain: domain.to_string(),
                domain_type,
                reason: Some("Domain is already registered to another publication".to_string()),
            });
        }
    }

    Ok(DomainAvailabilityResponse {
        available: true,
        domain: domain.to_string(),
        domain_type,
        reason: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum_test_helper::TestClient;

    #[tokio::test]
    async fn test_create_subdomain() {
        // Test subdomain creation with valid request
    }

    #[tokio::test]
    async fn test_domain_verification() {
        // Test domain verification process
    }

    #[tokio::test]
    async fn test_domain_resolution() {
        // Test resolving domain to publication
    }
}