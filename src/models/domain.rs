use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Represents the status of a domain
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "domain_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum DomainStatus {
    /// Domain is pending initial setup
    Pending,
    /// Domain is being verified
    Verifying,
    /// Domain is active and verified
    Active,
    /// Domain verification or setup failed
    Failed,
}

/// Represents the type of domain
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "domain_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum DomainType {
    /// Subdomain under the main platform domain
    Subdomain,
    /// Custom domain owned by the user
    Custom,
}

/// SSL certificate status for a domain
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "ssl_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum SSLStatus {
    /// No SSL certificate
    None,
    /// SSL certificate is being provisioned
    Pending,
    /// SSL certificate is active
    Active,
    /// SSL certificate has expired
    Expired,
    /// SSL certificate provisioning failed
    Failed,
}

/// Main domain model for publications
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PublicationDomain {
    pub id: Uuid,
    pub publication_id: Uuid,
    pub domain_type: DomainType,
    pub subdomain: Option<String>,
    pub custom_domain: Option<String>,
    pub status: DomainStatus,
    pub verification_token: Option<String>,
    pub verified_at: Option<DateTime<Utc>>,
    pub ssl_status: SSLStatus,
    pub ssl_expires_at: Option<DateTime<Utc>>,
    pub is_primary: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// DNS verification record for custom domains
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainVerificationRecord {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub record_type: String, // TXT, CNAME, etc.
    pub record_name: String,
    pub record_value: String,
    pub is_verified: bool,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request to create a new subdomain
#[derive(Debug, Deserialize)]
pub struct CreateSubdomainRequest {
    pub subdomain: String,
    pub is_primary: Option<bool>,
}

/// Request to add a custom domain
#[derive(Debug, Deserialize)]
pub struct AddCustomDomainRequest {
    pub domain: String,
    pub is_primary: Option<bool>,
}

/// Request to verify a domain
#[derive(Debug, Deserialize)]
pub struct VerifyDomainRequest {
    pub domain_id: Uuid,
}

/// Request to update domain settings
#[derive(Debug, Deserialize)]
pub struct UpdateDomainRequest {
    pub is_primary: Option<bool>,
    pub ssl_enabled: Option<bool>,
}

/// Response for domain creation
#[derive(Debug, Serialize)]
pub struct DomainResponse {
    pub domain: PublicationDomain,
    pub verification_records: Option<Vec<DomainVerificationRecord>>,
}

/// Response for domain verification status
#[derive(Debug, Serialize)]
pub struct DomainVerificationResponse {
    pub domain_id: Uuid,
    pub status: DomainStatus,
    pub verification_records: Vec<DomainVerificationRecord>,
    pub verified: bool,
    pub errors: Option<Vec<String>>,
}

/// Response for domain list
#[derive(Debug, Serialize)]
pub struct DomainListResponse {
    pub domains: Vec<PublicationDomain>,
    pub total: i64,
}

/// Domain availability check response
#[derive(Debug, Serialize)]
pub struct DomainAvailabilityResponse {
    pub available: bool,
    pub domain: String,
    pub domain_type: DomainType,
    pub reason: Option<String>,
}

/// SSL certificate information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SSLCertificateInfo {
    pub domain_id: Uuid,
    pub status: SSLStatus,
    pub issuer: Option<String>,
    pub issued_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub auto_renew: bool,
    pub last_renewal_attempt: Option<DateTime<Utc>>,
}

/// Request to check domain availability
#[derive(Debug, Deserialize)]
pub struct CheckDomainAvailabilityRequest {
    pub domain: String,
    pub domain_type: DomainType,
}

/// Domain statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct DomainStats {
    pub total_domains: i64,
    pub active_domains: i64,
    pub pending_domains: i64,
    pub failed_domains: i64,
    pub ssl_active: i64,
    pub ssl_pending: i64,
}

impl PublicationDomain {
    /// Check if the domain is verified
    pub fn is_verified(&self) -> bool {
        self.status == DomainStatus::Active && self.verified_at.is_some()
    }

    /// Get the full domain URL
    pub fn get_full_url(&self, use_https: bool) -> String {
        let protocol = if use_https { "https" } else { "http" };
        let domain = match self.domain_type {
            DomainType::Subdomain => self.subdomain.as_ref().unwrap(),
            DomainType::Custom => self.custom_domain.as_ref().unwrap(),
        };
        format!("{}://{}", protocol, domain)
    }

    /// Check if SSL is enabled and active
    pub fn has_active_ssl(&self) -> bool {
        self.ssl_status == SSLStatus::Active
    }

    /// Check if domain needs SSL renewal
    pub fn needs_ssl_renewal(&self) -> bool {
        if let Some(expires_at) = self.ssl_expires_at {
            let days_until_expiry = (expires_at - Utc::now()).num_days();
            days_until_expiry <= 30 // Renew if expires in 30 days or less
        } else {
            false
        }
    }
}

impl DomainVerificationRecord {
    /// Check if the record needs re-verification
    pub fn needs_verification(&self) -> bool {
        if !self.is_verified {
            return true;
        }
        
        if let Some(last_checked) = self.last_checked_at {
            let hours_since_check = (Utc::now() - last_checked).num_hours();
            hours_since_check >= 24 // Re-verify every 24 hours
        } else {
            true
        }
    }
}

/// Validation helpers
impl CreateSubdomainRequest {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // Validate subdomain format
        if self.subdomain.is_empty() {
            errors.push("Subdomain cannot be empty".to_string());
        }

        if self.subdomain.len() < 3 {
            errors.push("Subdomain must be at least 3 characters long".to_string());
        }

        if self.subdomain.len() > 63 {
            errors.push("Subdomain cannot exceed 63 characters".to_string());
        }

        // Check for valid characters (alphanumeric and hyphens)
        if !self.subdomain.chars().all(|c| c.is_alphanumeric() || c == '-') {
            errors.push("Subdomain can only contain letters, numbers, and hyphens".to_string());
        }

        // Cannot start or end with hyphen
        if self.subdomain.starts_with('-') || self.subdomain.ends_with('-') {
            errors.push("Subdomain cannot start or end with a hyphen".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl AddCustomDomainRequest {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.domain.is_empty() {
            errors.push("Domain cannot be empty".to_string());
        }

        // Basic domain format validation
        let parts: Vec<&str> = self.domain.split('.').collect();
        if parts.len() < 2 {
            errors.push("Invalid domain format".to_string());
        }

        // Check each part of the domain
        for part in &parts {
            if part.is_empty() {
                errors.push("Domain parts cannot be empty".to_string());
                break;
            }
            if !part.chars().all(|c| c.is_alphanumeric() || c == '-') {
                errors.push("Domain parts can only contain letters, numbers, and hyphens".to_string());
                break;
            }
            if part.starts_with('-') || part.ends_with('-') {
                errors.push("Domain parts cannot start or end with a hyphen".to_string());
                break;
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subdomain_validation() {
        let valid_subdomain = CreateSubdomainRequest {
            subdomain: "my-blog".to_string(),
            is_primary: Some(true),
        };
        assert!(valid_subdomain.validate().is_ok());

        let invalid_subdomain = CreateSubdomainRequest {
            subdomain: "-invalid".to_string(),
            is_primary: Some(false),
        };
        assert!(invalid_subdomain.validate().is_err());
    }

    #[test]
    fn test_custom_domain_validation() {
        let valid_domain = AddCustomDomainRequest {
            domain: "example.com".to_string(),
            is_primary: Some(true),
        };
        assert!(valid_domain.validate().is_ok());

        let invalid_domain = AddCustomDomainRequest {
            domain: "invalid".to_string(),
            is_primary: Some(false),
        };
        assert!(invalid_domain.validate().is_err());
    }

    #[test]
    fn test_domain_url_generation() {
        let subdomain = PublicationDomain {
            id: Uuid::new_v4(),
            publication_id: Uuid::new_v4(),
            domain_type: DomainType::Subdomain,
            subdomain: Some("myblog.platform.com".to_string()),
            custom_domain: None,
            status: DomainStatus::Active,
            verification_token: None,
            verified_at: Some(Utc::now()),
            ssl_status: SSLStatus::Active,
            ssl_expires_at: Some(Utc::now() + chrono::Duration::days(90)),
            is_primary: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert_eq!(subdomain.get_full_url(true), "https://myblog.platform.com");
        assert_eq!(subdomain.get_full_url(false), "http://myblog.platform.com");
    }
}