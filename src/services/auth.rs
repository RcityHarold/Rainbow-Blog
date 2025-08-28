use crate::{config::Config, error::{AppError, Result}};
use axum::{
    async_trait,
    extract::{FromRequestParts, State},
    headers::{authorization::Bearer, Authorization},
    http::{request::Parts, StatusCode},
    Extension,
    RequestPartsExt, TypedHeader,
};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc, Duration};
use tracing::{info, warn, error, debug};

#[derive(Clone)]
pub struct AuthService {
    config: Config,
    http_client: Client,
    user_cache: Arc<RwLock<HashMap<String, CachedUser>>>,
    permission_cache: Arc<RwLock<HashMap<String, CachedPermission>>>,
}

#[derive(Debug, Clone)]
struct CachedUser {
    user: User,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct CachedPermission {
    has_permission: bool,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,        // 用户ID
    pub exp: i64,           // 过期时间
    pub iat: i64,           // 签发时间
    pub session_id: Option<String>, // 会话ID
    pub email: Option<String>,      // 邮箱
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub is_verified: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RainbowAuthUserResponse {
    pub id: String,
    pub email: String,
    pub username: Option<String>,
    pub email_verified: bool,
    pub created_at: String,
    pub profile: Option<UserProfileResponse>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserProfileResponse {
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RainbowAuthPermissionResponse {
    pub success: bool,
    pub data: PermissionData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PermissionData {
    pub has_permission: bool,
    pub user_id: String,
    pub permission: String,
}

impl AuthService {
    pub async fn new(config: &Config) -> Result<Self> {
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| AppError::Internal(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            config: config.clone(),
            http_client,
            user_cache: Arc::new(RwLock::new(HashMap::new())),
            permission_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub fn verify_jwt(&self, token: &str) -> Result<Claims> {
        let decoding_key = DecodingKey::from_secret(self.config.jwt_secret.as_ref());
        let validation = Validation::new(Algorithm::HS256);

        match decode::<Claims>(token, &decoding_key, &validation) {
            Ok(token_data) => {
                debug!("JWT token verified for user: {}", token_data.claims.sub);
                Ok(token_data.claims)
            }
            Err(e) => {
                warn!("JWT verification failed: {}", e);
                Err(AppError::Authentication("Invalid token".to_string()))
            }
        }
    }

    pub async fn get_user_from_rainbow_auth(&self, user_id: &str, token: &str) -> Result<User> {
        // 检查缓存
        if let Some(cached_user) = self.get_cached_user(user_id).await {
            debug!("Using cached user data for user: {}", user_id);
            return Ok(cached_user);
        }

        // 调用 Rainbow-Auth 获取用户信息
        let url = format!("{}/api/users/me", self.config.auth_service_url);
        
        let response = self.http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| {
                error!("Failed to fetch user from Rainbow-Auth: {}", e);
                AppError::ExternalService("Failed to verify user with Rainbow-Auth".to_string())
            })?;

        if !response.status().is_success() {
            warn!("Rainbow-Auth returned error status: {}", response.status());
            return Err(AppError::Authentication("Invalid credentials".to_string()));
        }

        let user_data: RainbowAuthUserResponse = response.json().await
            .map_err(|e| {
                error!("Failed to parse Rainbow-Auth response: {}", e);
                AppError::Authentication("Invalid response from Rainbow-Auth".to_string())
            })?;

        // 获取用户权限（为博客系统定制）
        let permissions = self.get_blog_permissions(&user_data.id, token).await?;

        let user = User {
            id: user_data.id.clone(),
            email: user_data.email,
            username: user_data.username,
            display_name: user_data.profile.as_ref().and_then(|p| p.display_name.clone()),
            avatar_url: user_data.profile.as_ref().and_then(|p| p.avatar_url.clone()),
            roles: vec!["user".to_string()], // 基础角色
            permissions,
            is_verified: user_data.email_verified,
            created_at: chrono::DateTime::parse_from_rfc3339(&user_data.created_at)
                .unwrap_or_else(|_| Utc::now().into())
                .with_timezone(&Utc),
        };

        // 缓存用户数据
        self.cache_user(&user_data.id, user.clone()).await;

        Ok(user)
    }

    async fn get_cached_user(&self, user_id: &str) -> Option<User> {
        let cache = self.user_cache.read().await;
        if let Some(cached) = cache.get(user_id) {
            if cached.expires_at > Utc::now() {
                return Some(cached.user.clone());
            }
        }
        None
    }

    async fn cache_user(&self, user_id: &str, user: User) {
        let mut cache = self.user_cache.write().await;
        cache.insert(user_id.to_string(), CachedUser {
            user,
            expires_at: Utc::now() + Duration::minutes(15), // 缓存15分钟
        });
    }

    // 为博客系统获取用户权限
    async fn get_blog_permissions(&self, user_id: &str, token: &str) -> Result<Vec<String>> {
        let mut permissions = vec![
            "article.read".to_string(),
            "comment.read".to_string(),
            "user.read_profile".to_string(),
        ];

        // 默认给所有认证用户写权限（可以根据实际需求调整）
        permissions.extend_from_slice(&[
            "article.write".to_string(),
            "article.create".to_string(),
            "comment.create".to_string(),
            "user.update_profile".to_string(),
        ]);

        // 可以根据用户角色或其他条件添加更多权限
        // 这里简化处理，实际可以调用 Rainbow-Auth 的 RBAC API
        
        Ok(permissions)
    }

    pub async fn check_permission(&self, user_id: &str, permission: &str) -> Result<bool> {
        // 检查权限缓存
        let cache_key = format!("{}:{}", user_id, permission);
        if let Some(cached_permission) = self.get_cached_permission(&cache_key).await {
            debug!("Using cached permission for {}: {}", cache_key, cached_permission);
            return Ok(cached_permission);
        }

        // 博客系统权限检查逻辑
        let has_permission = match permission {
            // 读取权限（所有认证用户）
            "article.read" | "comment.read" | "user.read_profile" | "tag.read" => true,
            
            // 写入权限（认证用户）
            "article.create" | "article.update" | "comment.create" | "user.update_profile" => true,
            
            // 删除权限（作者本人或管理员）
            "article.delete" | "comment.delete" => true, // 简化处理，实际需要检查所有权
            
            // 管理权限
            "publication.create" | "publication.manage" => true, // 可以后续细化
            
            _ => false,
        };
        
        // 缓存权限结果
        self.cache_permission(&cache_key, has_permission).await;

        Ok(has_permission)
    }

    async fn get_cached_permission(&self, cache_key: &str) -> Option<bool> {
        let cache = self.permission_cache.read().await;
        if let Some(cached) = cache.get(cache_key) {
            if cached.expires_at > Utc::now() {
                return Some(cached.has_permission);
            }
        }
        None
    }

    async fn cache_permission(&self, cache_key: &str, has_permission: bool) {
        let mut cache = self.permission_cache.write().await;
        cache.insert(cache_key.to_string(), CachedPermission {
            has_permission,
            expires_at: Utc::now() + Duration::minutes(10), // 权限缓存10分钟
        });
    }

    // 检查用户是否为文章作者
    pub async fn check_article_ownership(&self, user_id: &str, article_author_id: &str) -> bool {
        user_id == article_author_id
    }

    // 检查用户是否为评论作者
    pub async fn check_comment_ownership(&self, user_id: &str, comment_author_id: &str) -> bool {
        user_id == comment_author_id
    }

    // 清理过期缓存
    pub async fn cleanup_expired_sessions(&self) -> Result<()> {
        let now = Utc::now();
        
        // 清理用户缓存
        {
            let mut user_cache = self.user_cache.write().await;
            let before_count = user_cache.len();
            user_cache.retain(|_, cached| cached.expires_at > now);
            let after_count = user_cache.len();
            debug!("Cleaned {} expired user cache entries", before_count - after_count);
        }
        
        // 清理权限缓存  
        {
            let mut permission_cache = self.permission_cache.write().await;
            let before_count = permission_cache.len();
            permission_cache.retain(|_, cached| cached.expires_at > now);
            let after_count = permission_cache.len();
            debug!("Cleaned {} expired permission cache entries", before_count - after_count);
        }
        
        info!("Authentication cache cleanup completed");
        Ok(())
    }

    // 获取当前在线用户数（通过缓存估算）
    pub async fn get_active_user_count(&self) -> usize {
        let cache = self.user_cache.read().await;
        let now = Utc::now();
        cache.values()
            .filter(|cached| cached.expires_at > now)
            .count()
    }
}

// Axum extractor for authentication
#[async_trait]
impl<S> FromRequestParts<S> for User
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self> {
        // 从请求头中提取 Authorization
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| AppError::Authentication("Missing authorization header".to_string()))?;

        // 从应用状态中获取认证服务
        let Extension(auth_service): Extension<Arc<AuthService>> = parts
            .extract::<Extension<Arc<AuthService>>>()
            .await
            .map_err(|_| AppError::Internal("Auth service not found in request extensions".to_string()))?;

        // 验证 JWT token
        let claims = auth_service.verify_jwt(bearer.token())?;

        // 从 Rainbow-Auth 获取用户详细信息
        auth_service.get_user_from_rainbow_auth(&claims.sub, bearer.token()).await
    }
}

// Optional authentication extractor
pub struct OptionalUser(pub Option<User>);

#[async_trait]
impl<S> FromRequestParts<S> for OptionalUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self> {
        match User::from_request_parts(parts, state).await {
            Ok(user) => Ok(OptionalUser(Some(user))),
            Err(_) => Ok(OptionalUser(None)),
        }
    }
}

// 权限检查的辅助宏
#[macro_export]
macro_rules! require_permission {
    ($auth_service:expr, $user:expr, $permission:expr) => {
        if !$auth_service.check_permission(&$user.id, $permission).await? {
            return Err(AppError::Authorization(format!("Permission '{}' required", $permission)));
        }
    };
}

// 检查文章所有权的辅助宏
#[macro_export]
macro_rules! require_article_ownership {
    ($auth_service:expr, $user:expr, $article_author_id:expr) => {
        if !$auth_service.check_article_ownership(&$user.id, $article_author_id).await {
            return Err(AppError::Authorization("Only article author can perform this action".to_string()));
        }
    };
}