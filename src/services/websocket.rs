use crate::{
    error::{AppError, Result},
    models::websocket::*,
    services::Database,
};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, info, warn, error};
use axum::extract::ws::{WebSocket, Message};
use futures::{sink::SinkExt, stream::StreamExt};

/// WebSocket连接管理器
#[derive(Clone)]
pub struct WebSocketService {
    db: Arc<Database>,
    // 连接管理
    connections: Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    // 用户到连接的映射
    user_connections: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    // 频道订阅管理
    channel_subscriptions: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    // 消息广播通道
    broadcast_tx: broadcast::Sender<WebSocketMessage>,
    // 消息队列发送端
    message_queue_tx: mpsc::UnboundedSender<MessageQueueItem>,
}

/// 连接信息
#[derive(Debug, Clone)]
struct ConnectionInfo {
    connection_id: String,
    user_id: String,
    tx: mpsc::UnboundedSender<WebSocketMessage>,
    connected_at: DateTime<Utc>,
    last_ping_at: DateTime<Utc>,
    subscriptions: HashSet<String>,
}

impl WebSocketService {
    pub async fn new(db: Arc<Database>) -> Result<Self> {
        let (broadcast_tx, _) = broadcast::channel(10000);
        let (message_queue_tx, mut message_queue_rx) = mpsc::unbounded_channel();
        
        let service = Self {
            db: db.clone(),
            connections: Arc::new(RwLock::new(HashMap::new())),
            user_connections: Arc::new(RwLock::new(HashMap::new())),
            channel_subscriptions: Arc::new(RwLock::new(HashMap::new())),
            broadcast_tx,
            message_queue_tx,
        };

        // 启动消息队列处理器
        let service_clone = service.clone();
        tokio::spawn(async move {
            while let Some(queue_item) = message_queue_rx.recv().await {
                if let Err(e) = service_clone.process_queue_message(queue_item).await {
                    error!("Failed to process queue message: {}", e);
                }
            }
        });

        // 启动清理任务
        let service_clone = service.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                service_clone.cleanup_stale_connections().await;
            }
        });

        Ok(service)
    }

    /// 处理新的WebSocket连接
    pub async fn handle_connection(
        &self,
        websocket: WebSocket,
        user_id: String,
        connection_id: String,
    ) -> Result<()> {
        info!("New WebSocket connection: {} for user: {}", connection_id, user_id);

        let (mut ws_tx, mut ws_rx) = websocket.split();
        let (tx, mut rx) = mpsc::unbounded_channel();

        // 创建连接信息
        let connection_info = ConnectionInfo {
            connection_id: connection_id.clone(),
            user_id: user_id.clone(),
            tx: tx.clone(),
            connected_at: Utc::now(),
            last_ping_at: Utc::now(),
            subscriptions: HashSet::new(),
        };

        // 注册连接
        self.register_connection(connection_info).await;

        // 发送连接确认消息
        let connect_msg = WebSocketMessage::new(
            WebSocketMessageType::Connect,
            json!({
                "connection_id": connection_id,
                "user_id": user_id,
                "timestamp": Utc::now()
            })
        );
        
        if let Err(e) = tx.send(connect_msg) {
            error!("Failed to send connect message: {}", e);
        }

        // 处理发送消息任务
        let connection_id_clone = connection_id.clone();
        let send_task = tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                match serde_json::to_string(&message) {
                    Ok(json_str) => {
                        if let Err(e) = ws_tx.send(Message::Text(json_str)).await {
                            error!("Failed to send WebSocket message: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Failed to serialize message: {}", e);
                    }
                }
            }
            debug!("Send task ended for connection: {}", connection_id_clone);
        });

        // 处理接收消息任务
        let service_clone = self.clone();
        let user_id_clone = user_id.clone();
        let connection_id_clone = connection_id.clone();
        let receive_task = tokio::spawn(async move {
            while let Some(msg_result) = ws_rx.next().await {
                match msg_result {
                    Ok(Message::Text(text)) => {
                        if let Err(e) = service_clone.handle_incoming_message(&connection_id_clone, &user_id_clone, text).await {
                            error!("Error handling incoming message: {}", e);
                        }
                    }
                    Ok(Message::Binary(data)) => {
                        debug!("Received binary message of {} bytes", data.len());
                    }
                    Ok(Message::Close(_)) => {
                        info!("WebSocket connection closed: {}", connection_id_clone);
                        break;
                    }
                    Ok(Message::Ping(data)) => {
                        debug!("Received ping from connection: {}", connection_id_clone);
                        service_clone.handle_ping(&connection_id_clone, data).await;
                    }
                    Ok(Message::Pong(_)) => {
                        service_clone.handle_pong(&connection_id_clone).await;
                    }
                    Err(e) => {
                        error!("WebSocket error for connection {}: {}", connection_id_clone, e);
                        break;
                    }
                }
            }
            debug!("Receive task ended for connection: {}", connection_id_clone);
        });

        // 等待任务结束
        let _ = tokio::try_join!(send_task, receive_task);

        // 清理连接
        self.unregister_connection(&connection_id, &user_id).await;

        Ok(())
    }

    /// 注册新连接
    async fn register_connection(&self, connection_info: ConnectionInfo) {
        let connection_id = connection_info.connection_id.clone();
        let user_id = connection_info.user_id.clone();

        // 添加到连接表
        {
            let mut connections = self.connections.write().unwrap();
            connections.insert(connection_id.clone(), connection_info);
        }

        // 更新用户连接映射
        {
            let mut user_connections = self.user_connections.write().unwrap();
            user_connections
                .entry(user_id.clone())
                .or_insert_with(HashSet::new)
                .insert(connection_id.clone());
        }

        // 自动订阅用户通知频道
        let user_channel = ChannelType::UserNotifications.channel_name(&user_id);
        self.subscribe_to_channel(&connection_id, &user_channel).await;

        debug!("Registered connection: {} for user: {}", connection_id, user_id);
    }

    /// 注销连接
    async fn unregister_connection(&self, connection_id: &str, user_id: &str) {
        // 从连接表移除
        let subscriptions = {
            let mut connections = self.connections.write().unwrap();
            connections.remove(connection_id)
                .map(|conn| conn.subscriptions)
                .unwrap_or_default()
        };

        // 更新用户连接映射
        {
            let mut user_connections = self.user_connections.write().unwrap();
            if let Some(user_conns) = user_connections.get_mut(user_id) {
                user_conns.remove(connection_id);
                if user_conns.is_empty() {
                    user_connections.remove(user_id);
                }
            }
        }

        // 清理频道订阅
        for channel in subscriptions {
            self.unsubscribe_from_channel(connection_id, &channel).await;
        }

        info!("Unregistered connection: {} for user: {}", connection_id, user_id);
    }

    /// 处理传入消息
    async fn handle_incoming_message(
        &self,
        connection_id: &str,
        user_id: &str,
        text: String,
    ) -> Result<()> {
        debug!("Received message from {}: {}", connection_id, text);

        let message: WebSocketMessage = serde_json::from_str(&text)
            .map_err(|e| AppError::BadRequest(format!("Invalid JSON: {}", e)))?;

        match message.message_type {
            WebSocketMessageType::Ping => {
                self.handle_ping_message(connection_id, message).await;
            }
            WebSocketMessageType::Subscribe => {
                self.handle_subscribe_message(connection_id, user_id, message).await?;
            }
            WebSocketMessageType::Unsubscribe => {
                self.handle_unsubscribe_message(connection_id, message).await?;
            }
            _ => {
                warn!("Unhandled message type: {:?}", message.message_type);
            }
        }

        Ok(())
    }

    /// 处理Ping消息
    async fn handle_ping_message(&self, connection_id: &str, message: WebSocketMessage) {
        let client_timestamp = message.data.get("timestamp")
            .and_then(|ts| ts.as_str())
            .and_then(|ts| DateTime::parse_from_rfc3339(ts).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let pong_msg = WebSocketMessage::pong(connection_id, client_timestamp);
        
        if let Err(e) = self.send_to_connection(connection_id, pong_msg).await {
            error!("Failed to send pong message: {}", e);
        }
    }

    /// 处理订阅消息
    async fn handle_subscribe_message(
        &self,
        connection_id: &str,
        user_id: &str,
        message: WebSocketMessage,
    ) -> Result<()> {
        let subscribe_req: SubscribeRequest = serde_json::from_value(message.data)
            .map_err(|e| AppError::BadRequest(format!("Invalid subscribe request: {}", e)))?;

        let mut subscribed_channels = Vec::new();

        for channel in subscribe_req.channels {
            if self.can_subscribe_to_channel(user_id, &channel).await? {
                self.subscribe_to_channel(connection_id, &channel).await;
                subscribed_channels.push(channel);
            } else {
                warn!("User {} not authorized to subscribe to channel: {}", user_id, channel);
            }
        }

        // 发送订阅确认
        let ack_msg = WebSocketMessage::new(
            WebSocketMessageType::SubscribeAck,
            json!({
                "subscribed_channels": subscribed_channels,
                "timestamp": Utc::now()
            })
        );

        self.send_to_connection(connection_id, ack_msg).await?;
        Ok(())
    }

    /// 处理取消订阅消息
    async fn handle_unsubscribe_message(
        &self,
        connection_id: &str,
        message: WebSocketMessage,
    ) -> Result<()> {
        let unsubscribe_req: UnsubscribeRequest = serde_json::from_value(message.data)
            .map_err(|e| AppError::BadRequest(format!("Invalid unsubscribe request: {}", e)))?;

        for channel in &unsubscribe_req.channels {
            self.unsubscribe_from_channel(connection_id, channel).await;
        }

        // 发送取消订阅确认
        let ack_msg = WebSocketMessage::new(
            WebSocketMessageType::UnsubscribeAck,
            json!({
                "unsubscribed_channels": unsubscribe_req.channels,
                "timestamp": Utc::now()
            })
        );

        self.send_to_connection(connection_id, ack_msg).await?;
        Ok(())
    }

    /// 订阅频道
    async fn subscribe_to_channel(&self, connection_id: &str, channel: &str) {
        // 更新连接的订阅列表
        {
            let mut connections = self.connections.write().unwrap();
            if let Some(conn) = connections.get_mut(connection_id) {
                conn.subscriptions.insert(channel.to_string());
            }
        }

        // 更新频道订阅列表
        {
            let mut channel_subscriptions = self.channel_subscriptions.write().unwrap();
            channel_subscriptions
                .entry(channel.to_string())
                .or_insert_with(HashSet::new)
                .insert(connection_id.to_string());
        }

        debug!("Connection {} subscribed to channel: {}", connection_id, channel);
    }

    /// 取消订阅频道
    async fn unsubscribe_from_channel(&self, connection_id: &str, channel: &str) {
        // 更新连接的订阅列表
        {
            let mut connections = self.connections.write().unwrap();
            if let Some(conn) = connections.get_mut(connection_id) {
                conn.subscriptions.remove(channel);
            }
        }

        // 更新频道订阅列表
        {
            let mut channel_subscriptions = self.channel_subscriptions.write().unwrap();
            if let Some(subscribers) = channel_subscriptions.get_mut(channel) {
                subscribers.remove(connection_id);
                if subscribers.is_empty() {
                    channel_subscriptions.remove(channel);
                }
            }
        }

        debug!("Connection {} unsubscribed from channel: {}", connection_id, channel);
    }

    /// 检查是否可以订阅频道
    async fn can_subscribe_to_channel(&self, user_id: &str, channel: &str) -> Result<bool> {
        // 基本权限检查
        if channel.starts_with("user_notifications:") || channel.starts_with("user_activity:") {
            let channel_user_id = channel.split(':').nth(1).unwrap_or("");
            return Ok(channel_user_id == user_id);
        }

        if channel.starts_with("creator_revenue:") {
            let channel_creator_id = channel.split(':').nth(1).unwrap_or("");
            // TODO: 检查用户是否为该创作者
            return Ok(channel_creator_id == user_id);
        }

        // 公共频道
        if channel == "system_updates" || channel == "global_activity" {
            return Ok(true);
        }

        // 其他频道默认允许
        Ok(true)
    }

    /// 发送消息到指定连接
    pub async fn send_to_connection(
        &self,
        connection_id: &str,
        message: WebSocketMessage,
    ) -> Result<()> {
        let tx = {
            let connections = self.connections.read().unwrap();
            connections.get(connection_id).map(|conn| conn.tx.clone())
        };
        
        if let Some(tx) = tx {
            if let Err(_) = tx.send(message) {
                warn!("Failed to send message to connection: {}", connection_id);
                return Err(AppError::Internal("Connection send failed".to_string()));
            }
        } else {
            return Err(AppError::NotFound(format!("Connection not found: {}", connection_id)));
        }
        Ok(())
    }

    /// 发送消息到用户的所有连接
    pub async fn send_to_user(&self, user_id: &str, message: WebSocketMessage) -> Result<()> {
        let connection_ids = {
            let user_connections = self.user_connections.read().unwrap();
            user_connections.get(user_id).cloned()
        };
        
        if let Some(connection_ids) = connection_ids {
            for connection_id in connection_ids {
                if let Err(e) = self.send_to_connection(&connection_id, message.clone()).await {
                    warn!("Failed to send message to user {} connection {}: {}", user_id, connection_id, e);
                }
            }
        }
        Ok(())
    }

    /// 广播消息到频道
    pub async fn broadcast_to_channel(&self, channel: &str, message: WebSocketMessage) -> Result<()> {
        let subscribers = {
            let channel_subscriptions = self.channel_subscriptions.read().unwrap();
            channel_subscriptions.get(channel).cloned()
        };
        
        if let Some(subscribers) = subscribers {
            debug!("Broadcasting to channel {} with {} subscribers", channel, subscribers.len());
            
            for connection_id in subscribers {
                if let Err(e) = self.send_to_connection(&connection_id, message.clone()).await {
                    warn!("Failed to broadcast to connection {}: {}", connection_id, e);
                }
            }
        }
        Ok(())
    }

    /// 处理Ping
    async fn handle_ping(&self, connection_id: &str, _data: Vec<u8>) {
        self.update_last_ping(connection_id).await;
    }

    /// 处理Pong
    async fn handle_pong(&self, connection_id: &str) {
        self.update_last_ping(connection_id).await;
    }

    /// 更新最后Ping时间
    async fn update_last_ping(&self, connection_id: &str) {
        let mut connections = self.connections.write().unwrap();
        if let Some(conn) = connections.get_mut(connection_id) {
            conn.last_ping_at = Utc::now();
        }
    }

    /// 清理过期连接
    async fn cleanup_stale_connections(&self) {
        let threshold = Utc::now() - chrono::Duration::seconds(300); // 5分钟超时
        let mut stale_connections = Vec::new();

        {
            let connections = self.connections.read().unwrap();
            for (connection_id, conn) in connections.iter() {
                if conn.last_ping_at < threshold {
                    stale_connections.push((connection_id.clone(), conn.user_id.clone()));
                }
            }
        }

        for (connection_id, user_id) in stale_connections {
            warn!("Cleaning up stale connection: {} for user: {}", connection_id, user_id);
            self.unregister_connection(&connection_id, &user_id).await;
        }
    }

    /// 处理队列消息
    async fn process_queue_message(&self, queue_item: MessageQueueItem) -> Result<()> {
        debug!("Processing queue message: {}", queue_item.id);

        let message = queue_item.message;
        
        if let Some(user_id) = message.to_user_id.clone() {
            self.send_to_user(&user_id, message).await?;
        } else if let Some(channel) = message.channel.clone() {
            self.broadcast_to_channel(&channel, message).await?;
        }

        Ok(())
    }

    /// 获取在线统计
    pub async fn get_stats(&self) -> WebSocketStats {
        let connections = self.connections.read().unwrap();
        let channel_subscriptions = self.channel_subscriptions.read().unwrap();

        let total_connections = connections.len();
        let active_users = self.user_connections.read().unwrap().len();

        let channels = channel_subscriptions
            .iter()
            .map(|(channel, subscribers)| ChannelStats {
                channel: channel.clone(),
                subscriber_count: subscribers.len(),
                message_count_24h: 0, // TODO: 实现消息计数
                last_activity: Some(Utc::now()),
            })
            .collect();

        WebSocketStats {
            total_connections,
            active_users,
            channels,
            messages_sent_24h: 0, // TODO: 实现消息计数
            average_response_time_ms: 0.0, // TODO: 实现响应时间统计
        }
    }

    /// 获取用户在线状态
    pub async fn get_user_online_status(&self, user_id: &str) -> OnlineStatus {
        let user_connections = self.user_connections.read().unwrap();
        let connection_count = user_connections.get(user_id)
            .map(|conns| conns.len() as i32)
            .unwrap_or(0);

        OnlineStatus {
            user_id: user_id.to_string(),
            is_online: connection_count > 0,
            last_seen: Utc::now(), // TODO: 实现实际的最后活跃时间
            active_connections: connection_count,
        }
    }

    /// 添加消息到队列
    pub async fn queue_message(&self, message: WebSocketMessage, max_retries: i32) -> Result<()> {
        let queue_item = MessageQueueItem {
            id: format!("queue_{}", uuid::Uuid::new_v4()),
            message,
            retry_count: 0,
            max_retries,
            created_at: Utc::now(),
            scheduled_at: Utc::now(),
            processed_at: None,
        };

        self.message_queue_tx.send(queue_item)
            .map_err(|_| AppError::Internal("Failed to queue message".to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_websocket_service_creation() {
        // 需要实际的数据库连接进行完整测试
        // 这里只是基本的编译测试
    }

    #[test]
    fn test_channel_authorization() {
        // TODO: 添加频道权限测试
    }
}