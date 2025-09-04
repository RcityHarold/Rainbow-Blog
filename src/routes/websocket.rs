use axum::{
    extract::{
        ws::{WebSocket, WebSocketUpgrade},
        State, Path, Query,
    },
    response::{Json, Response},
    routing::{get, post},
    Router,
    Extension,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info, error, warn};

use crate::{
    error::{AppError, Result},
    models::websocket::*,
    services::auth::User,
    state::AppState,
};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        // WebSocket连接端点
        .route("/connect", get(websocket_handler))
        
        // 连接管理
        .route("/connections", get(list_connections))
        .route("/connections/:connection_id", get(get_connection))
        
        // 消息发送
        .route("/send", post(send_message))
        .route("/broadcast", post(broadcast_message))
        
        // 频道管理
        .route("/channels", get(list_channels))
        .route("/channels/:channel/subscribe", post(subscribe_channel))
        .route("/channels/:channel/unsubscribe", post(unsubscribe_channel))
        
        // 在线状态
        .route("/status/:user_id", get(get_user_status))
        .route("/online-users", get(list_online_users))
        
        // 统计信息
        .route("/stats", get(get_websocket_stats))
        
        // 通知配置
        .route("/config", get(get_notification_config))
        .route("/config", post(update_notification_config))
}

/// WebSocket连接处理器
async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
) -> Response {
    let connection_id = format!("conn_{}", uuid::Uuid::new_v4());
    
    info!("WebSocket upgrade request from user: {} with connection: {}", user.id, connection_id);
    
    ws.on_upgrade(move |socket| handle_websocket_connection(socket, state, user, connection_id))
}

/// 处理WebSocket连接
async fn handle_websocket_connection(
    socket: WebSocket,
    state: Arc<AppState>,
    user: User,
    connection_id: String,
) {
    info!("Handling WebSocket connection: {} for user: {}", connection_id, user.id);
    
    if let Err(e) = state.websocket_service
        .handle_connection(socket, user.id.clone(), connection_id.clone())
        .await 
    {
        error!("WebSocket connection error for {}: {}", connection_id, e);
    }
    
    info!("WebSocket connection closed: {} for user: {}", connection_id, user.id);
}

/// 获取连接列表
async fn list_connections(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
) -> Result<Json<serde_json::Value>> {
    debug!("Getting connections list for admin user: {}", user.id);
    
    // TODO: 检查管理员权限
    let stats = state.websocket_service.get_stats().await;
    
    Ok(Json(serde_json::json!({
        "success": true,
        "data": {
            "total_connections": stats.total_connections,
            "active_users": stats.active_users,
            "channels": stats.channels
        }
    })))
}

/// 获取连接详情
async fn get_connection(
    State(_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(_connection_id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    debug!("Getting connection details for user: {}", user.id);
    
    // TODO: 实现连接详情获取
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Connection details retrieved successfully"
    })))
}

#[derive(Debug, Deserialize)]
struct SendMessageRequest {
    message_type: WebSocketMessageType,
    data: serde_json::Value,
    to_user_id: Option<String>,
    channel: Option<String>,
}

/// 发送消息
async fn send_message(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Json(payload): Json<SendMessageRequest>,
) -> Result<Json<serde_json::Value>> {
    debug!("Sending message from user: {}", user.id);
    
    let mut message = WebSocketMessage::new(payload.message_type, payload.data);
    message.from_user_id = Some(user.id);
    message.to_user_id = payload.to_user_id.clone();
    message.channel = payload.channel.clone();
    
    match payload.to_user_id {
        Some(target_user_id) => {
            state.websocket_service
                .send_to_user(&target_user_id, message)
                .await?;
        }
        None => {
            if let Some(channel) = payload.channel {
                state.websocket_service
                    .broadcast_to_channel(&channel, message)
                    .await?;
            } else {
                return Err(AppError::BadRequest("Either to_user_id or channel must be specified".to_string()));
            }
        }
    }
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Message sent successfully"
    })))
}

#[derive(Debug, Deserialize)]
struct BroadcastMessageRequest {
    message_type: WebSocketMessageType,
    channel: String,
    data: serde_json::Value,
}

/// 广播消息
async fn broadcast_message(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Json(payload): Json<BroadcastMessageRequest>,
) -> Result<Json<serde_json::Value>> {
    debug!("Broadcasting message from user: {} to channel: {}", user.id, payload.channel);
    
    // TODO: 检查广播权限
    
    let message = WebSocketMessage::broadcast(
        payload.message_type,
        payload.channel.clone(),
        payload.data,
    );
    
    state.websocket_service
        .broadcast_to_channel(&payload.channel, message)
        .await?;
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Message broadcasted successfully"
    })))
}

/// 获取频道列表
async fn list_channels(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
) -> Result<Json<serde_json::Value>> {
    debug!("Getting channels list for user: {}", user.id);
    
    let stats = state.websocket_service.get_stats().await;
    let channels: Vec<_> = stats.channels.into_iter().collect();
    
    Ok(Json(serde_json::json!({
        "success": true,
        "data": {
            "channels": channels,
            "user_channels": [
                ChannelType::UserNotifications.channel_name(&user.id),
                ChannelType::UserActivity.channel_name(&user.id),
                ChannelType::CreatorUpdates.channel_name(&user.id),
                ChannelType::CreatorRevenue.channel_name(&user.id),
            ]
        }
    })))
}

/// 订阅频道
async fn subscribe_channel(
    State(_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(channel): Path<String>,
) -> Result<Json<serde_json::Value>> {
    debug!("User {} subscribing to channel: {}", user.id, channel);
    
    // TODO: 实现频道订阅逻辑
    // 这个功能通常通过WebSocket连接直接处理
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("Subscribed to channel: {}", channel)
    })))
}

/// 取消订阅频道
async fn unsubscribe_channel(
    State(_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Path(channel): Path<String>,
) -> Result<Json<serde_json::Value>> {
    debug!("User {} unsubscribing from channel: {}", user.id, channel);
    
    // TODO: 实现取消订阅逻辑
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("Unsubscribed from channel: {}", channel)
    })))
}

/// 获取用户在线状态
async fn get_user_status(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<User>,
    Path(target_user_id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    debug!("Getting online status for user: {}", target_user_id);
    
    let status = state.websocket_service
        .get_user_online_status(&target_user_id)
        .await;
    
    Ok(Json(serde_json::json!({
        "success": true,
        "data": status
    })))
}

#[derive(Debug, Deserialize)]
struct OnlineUsersQuery {
    limit: Option<i32>,
    offset: Option<i32>,
}

/// 获取在线用户列表
async fn list_online_users(
    State(_state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Query(_query): Query<OnlineUsersQuery>,
) -> Result<Json<serde_json::Value>> {
    debug!("Getting online users list for user: {}", user.id);
    
    // TODO: 实现在线用户列表获取
    let online_users = Vec::<OnlineStatus>::new();
    
    Ok(Json(serde_json::json!({
        "success": true,
        "data": {
            "online_users": online_users,
            "total_count": 0
        }
    })))
}

/// 获取WebSocket统计信息
async fn get_websocket_stats(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
) -> Result<Json<serde_json::Value>> {
    debug!("Getting WebSocket stats for user: {}", user.id);
    
    // TODO: 检查管理员权限
    
    let stats = state.websocket_service.get_stats().await;
    
    Ok(Json(serde_json::json!({
        "success": true,
        "data": stats
    })))
}

/// 获取通知配置
async fn get_notification_config(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
) -> Result<Json<serde_json::Value>> {
    debug!("Getting notification config for user: {}", user.id);
    
    // 从数据库获取用户通知配置
    let query = r#"
        SELECT * FROM notification_config WHERE user_id = $user_id LIMIT 1
    "#;

    let mut response = state.db.query_with_params(query, serde_json::json!({
        "user_id": user.id
    })).await?;

    let configs: Vec<serde_json::Value> = response.take(0)?;
    
    let config = if let Some(config_data) = configs.into_iter().next() {
        serde_json::from_value::<NotificationConfig>(config_data)
            .map_err(|e| AppError::Internal(format!("Failed to parse notification config: {}", e)))?
    } else {
        // 创建默认配置
        let default_config = NotificationConfig {
            user_id: user.id.clone(),
            email_notifications: true,
            push_notifications: true,
            websocket_notifications: true,
            notification_types: vec![
                "new_article".to_string(),
                "new_comment".to_string(),
                "new_follower".to_string(),
                "article_clap".to_string(),
                "subscription_update".to_string(),
                "payment_update".to_string(),
            ],
            quiet_hours_start: Some("22:00".to_string()),
            quiet_hours_end: Some("08:00".to_string()),
            timezone: "UTC".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        // 保存默认配置到数据库
        let create_query = r#"
            CREATE notification_config CONTENT $config
        "#;
        
        state.db.query_with_params(create_query, serde_json::json!({
            "config": default_config
        })).await?;
        
        default_config
    };
    
    Ok(Json(serde_json::json!({
        "success": true,
        "data": config
    })))
}

#[derive(Debug, Deserialize)]
struct UpdateNotificationConfigRequest {
    email_notifications: Option<bool>,
    push_notifications: Option<bool>,
    websocket_notifications: Option<bool>,
    notification_types: Option<Vec<String>>,
    quiet_hours_start: Option<String>,
    quiet_hours_end: Option<String>,
    timezone: Option<String>,
}

/// 更新通知配置
async fn update_notification_config(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<User>,
    Json(payload): Json<UpdateNotificationConfigRequest>,
) -> Result<Json<serde_json::Value>> {
    debug!("Updating notification config for user: {}", user.id);
    
    // 构建更新查询
    let mut updates = Vec::new();
    
    if let Some(email_notifications) = payload.email_notifications {
        updates.push(format!("email_notifications = {}", email_notifications));
    }
    if let Some(push_notifications) = payload.push_notifications {
        updates.push(format!("push_notifications = {}", push_notifications));
    }
    if let Some(websocket_notifications) = payload.websocket_notifications {
        updates.push(format!("websocket_notifications = {}", websocket_notifications));
    }
    if let Some(notification_types) = payload.notification_types {
        updates.push(format!("notification_types = {:?}", notification_types));
    }
    if let Some(quiet_hours_start) = payload.quiet_hours_start {
        updates.push(format!("quiet_hours_start = '{}'", quiet_hours_start));
    }
    if let Some(quiet_hours_end) = payload.quiet_hours_end {
        updates.push(format!("quiet_hours_end = '{}'", quiet_hours_end));
    }
    if let Some(timezone) = payload.timezone {
        updates.push(format!("timezone = '{}'", timezone));
    }
    
    if updates.is_empty() {
        return Err(AppError::BadRequest("No fields to update".to_string()));
    }
    
    updates.push("updated_at = $updated_at".to_string());
    
    let update_query = format!(
        "UPDATE notification_config SET {} WHERE user_id = $user_id",
        updates.join(", ")
    );
    
    state.db.query_with_params(&update_query, serde_json::json!({
        "user_id": user.id,
        "updated_at": chrono::Utc::now()
    })).await?;
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Notification config updated successfully"
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_message_request() {
        let request = SendMessageRequest {
            message_type: WebSocketMessageType::Notification,
            data: serde_json::json!({"test": "data"}),
            to_user_id: Some("user_123".to_string()),
            channel: None,
        };
        
        assert_eq!(request.message_type, WebSocketMessageType::Notification);
        assert_eq!(request.to_user_id, Some("user_123".to_string()));
    }
}