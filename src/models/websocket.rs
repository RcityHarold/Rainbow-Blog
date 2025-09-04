use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// WebSocket连接信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConnection {
    pub id: String,
    pub user_id: String,
    pub connection_id: String,
    pub connected_at: DateTime<Utc>,
    pub last_ping_at: DateTime<Utc>,
    pub subscriptions: Vec<String>, // 订阅的频道列表
    pub metadata: HashMap<String, String>,
}

/// WebSocket消息类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WebSocketMessageType {
    // 系统消息
    Connect,
    Disconnect,
    Ping,
    Pong,
    Error,
    
    // 订阅管理
    Subscribe,
    Unsubscribe,
    SubscribeAck,
    UnsubscribeAck,
    
    // 通知消息
    Notification,
    ArticleUpdate,
    CommentUpdate,
    UserUpdate,
    
    // 实时消息
    NewArticle,
    NewComment,
    NewFollower,
    NewClap,
    
    // 商业化消息
    SubscriptionUpdate,
    PaymentUpdate,
    RevenueUpdate,
    
    // 广播消息
    SystemAnnouncement,
    MaintenanceNotice,
}

/// WebSocket消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketMessage {
    pub id: String,
    pub message_type: WebSocketMessageType,
    pub channel: Option<String>,
    pub data: serde_json::Value,
    pub timestamp: DateTime<Utc>,
    pub from_user_id: Option<String>,
    pub to_user_id: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// 频道类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChannelType {
    // 用户频道
    UserNotifications,  // user_notifications:{user_id}
    UserActivity,      // user_activity:{user_id}
    
    // 文章频道
    ArticleComments,   // article_comments:{article_id}
    ArticleClaps,      // article_claps:{article_id}
    
    // 创作者频道
    CreatorUpdates,    // creator_updates:{creator_id}
    CreatorRevenue,    // creator_revenue:{creator_id}
    
    // 出版物频道
    PublicationUpdates, // publication_updates:{publication_id}
    
    // 系统频道
    SystemUpdates,     // system_updates
    GlobalActivity,    // global_activity
}

impl ChannelType {
    /// 生成频道名称
    pub fn channel_name(&self, id: &str) -> String {
        match self {
            ChannelType::UserNotifications => format!("user_notifications:{}", id),
            ChannelType::UserActivity => format!("user_activity:{}", id),
            ChannelType::ArticleComments => format!("article_comments:{}", id),
            ChannelType::ArticleClaps => format!("article_claps:{}", id),
            ChannelType::CreatorUpdates => format!("creator_updates:{}", id),
            ChannelType::CreatorRevenue => format!("creator_revenue:{}", id),
            ChannelType::PublicationUpdates => format!("publication_updates:{}", id),
            ChannelType::SystemUpdates => "system_updates".to_string(),
            ChannelType::GlobalActivity => "global_activity".to_string(),
        }
    }
}

/// 订阅请求
#[derive(Debug, Deserialize)]
pub struct SubscribeRequest {
    pub channels: Vec<String>,
    pub metadata: Option<HashMap<String, String>>,
}

/// 取消订阅请求
#[derive(Debug, Deserialize)]
pub struct UnsubscribeRequest {
    pub channels: Vec<String>,
}

/// 发送消息请求
#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub message_type: WebSocketMessageType,
    pub channel: Option<String>,
    pub data: serde_json::Value,
    pub to_user_id: Option<String>,
    pub metadata: Option<HashMap<String, String>>,
}

/// 在线状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineStatus {
    pub user_id: String,
    pub is_online: bool,
    pub last_seen: DateTime<Utc>,
    pub active_connections: i32,
}

/// 频道统计
#[derive(Debug, Serialize)]
pub struct ChannelStats {
    pub channel: String,
    pub subscriber_count: usize,
    pub message_count_24h: i64,
    pub last_activity: Option<DateTime<Utc>>,
}

/// WebSocket统计
#[derive(Debug, Serialize)]
pub struct WebSocketStats {
    pub total_connections: usize,
    pub active_users: usize,
    pub channels: Vec<ChannelStats>,
    pub messages_sent_24h: i64,
    pub average_response_time_ms: f64,
}

/// 实时通知配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub user_id: String,
    pub email_notifications: bool,
    pub push_notifications: bool,
    pub websocket_notifications: bool,
    pub notification_types: Vec<String>, // 启用的通知类型
    pub quiet_hours_start: Option<String>, // "22:00"
    pub quiet_hours_end: Option<String>,   // "08:00"
    pub timezone: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 消息队列项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageQueueItem {
    pub id: String,
    pub message: WebSocketMessage,
    pub retry_count: i32,
    pub max_retries: i32,
    pub created_at: DateTime<Utc>,
    pub scheduled_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
}

/// 连接心跳
#[derive(Debug, Serialize, Deserialize)]
pub struct HeartbeatMessage {
    pub connection_id: String,
    pub timestamp: DateTime<Utc>,
    pub client_timestamp: Option<DateTime<Utc>>,
}

/// 错误消息
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorMessage {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl WebSocketMessage {
    /// 创建新消息
    pub fn new(
        message_type: WebSocketMessageType,
        data: serde_json::Value,
    ) -> Self {
        Self {
            id: format!("msg_{}", uuid::Uuid::new_v4()),
            message_type,
            channel: None,
            data,
            timestamp: Utc::now(),
            from_user_id: None,
            to_user_id: None,
            metadata: HashMap::new(),
        }
    }
    
    /// 创建通知消息
    pub fn notification(
        data: serde_json::Value,
        to_user_id: String,
    ) -> Self {
        Self {
            id: format!("msg_{}", uuid::Uuid::new_v4()),
            message_type: WebSocketMessageType::Notification,
            channel: Some(ChannelType::UserNotifications.channel_name(&to_user_id)),
            data,
            timestamp: Utc::now(),
            from_user_id: None,
            to_user_id: Some(to_user_id),
            metadata: HashMap::new(),
        }
    }
    
    /// 创建广播消息
    pub fn broadcast(
        message_type: WebSocketMessageType,
        channel: String,
        data: serde_json::Value,
    ) -> Self {
        Self {
            id: format!("msg_{}", uuid::Uuid::new_v4()),
            message_type,
            channel: Some(channel),
            data,
            timestamp: Utc::now(),
            from_user_id: None,
            to_user_id: None,
            metadata: HashMap::new(),
        }
    }
    
    /// 创建错误消息
    pub fn error(code: &str, message: &str, details: Option<serde_json::Value>) -> Self {
        let error_data = ErrorMessage {
            code: code.to_string(),
            message: message.to_string(),
            details,
        };
        
        Self {
            id: format!("msg_{}", uuid::Uuid::new_v4()),
            message_type: WebSocketMessageType::Error,
            channel: None,
            data: serde_json::to_value(error_data).unwrap_or_default(),
            timestamp: Utc::now(),
            from_user_id: None,
            to_user_id: None,
            metadata: HashMap::new(),
        }
    }
    
    /// 创建心跳消息
    pub fn pong(connection_id: &str, client_timestamp: Option<DateTime<Utc>>) -> Self {
        let heartbeat = HeartbeatMessage {
            connection_id: connection_id.to_string(),
            timestamp: Utc::now(),
            client_timestamp,
        };
        
        Self {
            id: format!("msg_{}", uuid::Uuid::new_v4()),
            message_type: WebSocketMessageType::Pong,
            channel: None,
            data: serde_json::to_value(heartbeat).unwrap_or_default(),
            timestamp: Utc::now(),
            from_user_id: None,
            to_user_id: None,
            metadata: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_name_generation() {
        assert_eq!(
            ChannelType::UserNotifications.channel_name("user_123"),
            "user_notifications:user_123"
        );
        assert_eq!(
            ChannelType::ArticleComments.channel_name("article_456"),
            "article_comments:article_456"
        );
        assert_eq!(
            ChannelType::SystemUpdates.channel_name(""),
            "system_updates"
        );
    }

    #[test]
    fn test_websocket_message_creation() {
        let data = serde_json::json!({
            "title": "Test Notification",
            "message": "This is a test"
        });

        let message = WebSocketMessage::notification(data.clone(), "user_123".to_string());
        
        assert_eq!(message.message_type, WebSocketMessageType::Notification);
        assert_eq!(message.to_user_id, Some("user_123".to_string()));
        assert_eq!(message.channel, Some("user_notifications:user_123".to_string()));
        assert_eq!(message.data, data);
    }

    #[test]
    fn test_error_message_creation() {
        let error_msg = WebSocketMessage::error(
            "INVALID_CHANNEL",
            "Channel not found",
            Some(serde_json::json!({"channel": "invalid_channel"}))
        );
        
        assert_eq!(error_msg.message_type, WebSocketMessageType::Error);
        assert!(error_msg.data.get("code").is_some());
        assert!(error_msg.data.get("message").is_some());
    }
}