use crate::{
    error::{AppError, Result},
    models::publication::*,
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
        .route("/", get(get_publications).post(create_publication))
        .route("/:slug", get(get_publication).put(update_publication).delete(delete_publication))
        .route("/:slug/articles", get(get_publication_articles))
        .route("/:id/members", get(get_members).post(add_member))
        .route("/:id/members/:user_id", put(update_member).delete(remove_member))
        .route("/:id/follow", post(follow_publication).delete(unfollow_publication))
}

/// 获取出版物列表
/// GET /api/publications
async fn get_publications(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PublicationQuery>,
) -> Result<Json<Value>> {
    debug!("Getting publications list");

    let publications = state.publication_service.get_publications(query).await?;

    Ok(Json(json!({
        "success": true,
        "data": publications
    })))
}

/// 创建出版物
/// POST /api/publications
async fn create_publication(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Json(request): Json<CreatePublicationRequest>,
) -> Result<Json<Value>> {
    debug!("Creating publication: {} for user: {}", request.name, user.id);

    let publication = state
        .publication_service
        .create_publication(&user.id, request)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": publication,
        "message": "Publication created successfully"
    })))
}

/// 获取出版物详情
/// GET /api/publications/:slug
async fn get_publication(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    OptionalAuth(user): OptionalAuth,
) -> Result<Json<Value>> {
    debug!("Getting publication: {}", slug);

    let user_id = user.as_ref().map(|u| u.id.as_str());
    let publication = state
        .publication_service
        .get_publication(&slug, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Publication not found".to_string()))?;

    Ok(Json(json!({
        "success": true,
        "data": publication
    })))
}

/// 更新出版物
/// PUT /api/publications/:slug
async fn update_publication(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(slug): Path<String>,
    Json(request): Json<UpdatePublicationRequest>,
) -> Result<Json<Value>> {
    debug!("Updating publication: {} by user: {}", slug, user.id);

    // 先通过slug获取publication_id
    let existing = state
        .publication_service
        .get_publication(&slug, Some(&user.id))
        .await?
        .ok_or_else(|| AppError::NotFound("Publication not found".to_string()))?;

    let updated_publication = state
        .publication_service
        .update_publication(&existing.publication.id, &user.id, request)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": updated_publication,
        "message": "Publication updated successfully"
    })))
}

/// 删除出版物
/// DELETE /api/publications/:slug
async fn delete_publication(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(slug): Path<String>,
) -> Result<Json<Value>> {
    debug!("Deleting publication: {} by user: {}", slug, user.id);

    // 先通过slug获取publication_id
    let existing = state
        .publication_service
        .get_publication(&slug, Some(&user.id))
        .await?
        .ok_or_else(|| AppError::NotFound("Publication not found".to_string()))?;

    state
        .publication_service
        .delete_publication(&existing.publication.id, &user.id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "Publication deleted successfully"
    })))
}

/// 获取出版物文章
/// GET /api/publications/:slug/articles
async fn get_publication_articles(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    Query(pagination): Query<ArticlesPaginationQuery>,
) -> Result<Json<Value>> {
    debug!("Getting articles for publication: {}", slug);

    // 先通过slug获取publication_id
    let publication = state
        .publication_service
        .get_publication(&slug, None)
        .await?
        .ok_or_else(|| AppError::NotFound("Publication not found".to_string()))?;

    let page = pagination.page.unwrap_or(1);
    let limit = pagination.limit.unwrap_or(20);

    let articles = state
        .publication_service
        .get_publication_articles(&publication.publication.id, page, limit)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": articles
    })))
}

/// 添加成员
/// POST /api/publications/:id/members
async fn add_member(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(publication_id): Path<String>,
    Json(request): Json<AddMemberRequest>,
) -> Result<Json<Value>> {
    debug!("Adding member to publication: {}", publication_id);

    let member = state
        .publication_service
        .add_member(&publication_id, &user.id, request)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": member,
        "message": "Member added successfully"
    })))
}

/// 更新成员
/// PUT /api/publications/:id/members/:user_id
async fn update_member(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path((publication_id, member_user_id)): Path<(String, String)>,
    Json(request): Json<UpdateMemberRequest>,
) -> Result<Json<Value>> {
    debug!("Updating member in publication: {}", publication_id);

    let updated_member = state
        .publication_service
        .update_member(&publication_id, &member_user_id, &user.id, request)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": updated_member,
        "message": "Member updated successfully"
    })))
}

/// 移除成员
/// DELETE /api/publications/:id/members/:user_id
async fn remove_member(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path((publication_id, member_user_id)): Path<(String, String)>,
) -> Result<Json<Value>> {
    debug!("Removing member from publication: {}", publication_id);

    state
        .publication_service
        .remove_member(&publication_id, &member_user_id, &user.id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "Member removed successfully"
    })))
}

/// 获取成员列表
/// GET /api/publications/:id/members
async fn get_members(
    State(state): State<Arc<AppState>>,
    Path(publication_id): Path<String>,
    Query(pagination): Query<MembersPaginationQuery>,
) -> Result<Json<Value>> {
    debug!("Getting members for publication: {}", publication_id);

    let page = pagination.page.unwrap_or(1);
    let limit = pagination.limit.unwrap_or(20);

    let members = state
        .publication_service
        .get_members(&publication_id, page, limit)
        .await?;

    Ok(Json(json!({
        "success": true,
        "data": members
    })))
}

/// 关注出版物
/// POST /api/publications/:id/follow
async fn follow_publication(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(publication_id): Path<String>,
) -> Result<Json<Value>> {
    debug!("User {} following publication: {}", user.id, publication_id);

    state
        .publication_service
        .follow_publication(&publication_id, &user.id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "Publication followed successfully"
    })))
}

/// 取消关注出版物
/// DELETE /api/publications/:id/follow
async fn unfollow_publication(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(publication_id): Path<String>,
) -> Result<Json<Value>> {
    debug!("User {} unfollowing publication: {}", user.id, publication_id);

    state
        .publication_service
        .unfollow_publication(&publication_id, &user.id)
        .await?;

    Ok(Json(json!({
        "success": true,
        "message": "Publication unfollowed successfully"
    })))
}

#[derive(serde::Deserialize)]
struct ArticlesPaginationQuery {
    page: Option<usize>,
    limit: Option<usize>,
}

#[derive(serde::Deserialize)]
struct MembersPaginationQuery {
    page: Option<usize>,
    limit: Option<usize>,
}