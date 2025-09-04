use crate::{
    error::{AppError, Result},
    models::{
        websocket::*,
        notification::{CreateNotificationRequest, NotificationType},
        article::Article,
        comment::Comment,
    },
    services::{
        websocket::WebSocketService,
        notification::NotificationService,
    },
};
use chrono::{DateTime, Utc};
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, info, error};

/// 实时通知集成服务
/// 负责将传统通知与WebSocket实时推送整合
#[derive(Clone)]
pub struct RealtimeService {
    websocket_service: Arc<WebSocketService>,
    notification_service: Arc<NotificationService>,
}

impl RealtimeService {
    pub fn new(
        websocket_service: Arc<WebSocketService>,
        notification_service: Arc<NotificationService>,
    ) -> Self {
        Self {
            websocket_service,
            notification_service,
        }
    }

    /// 发送实时通知
    /// 同时处理传统通知和WebSocket推送
    pub async fn send_notification(
        &self,
        user_id: &str,
        notification_type: &str,
        title: &str,
        content: &str,
        data: Option<serde_json::Value>,
    ) -> Result<()> {
        debug!("Sending realtime notification to user: {} type: {}", user_id, notification_type);

        // 1. 发送传统通知（存储到数据库）
        let notification_request = crate::models::notification::CreateNotificationRequest {
            recipient_id: user_id.to_string(),
            notification_type: self.map_notification_type(notification_type),
            title: title.to_string(),
            message: content.to_string(),
            data: data.clone().unwrap_or_else(|| json!({})),
        };
        
        if let Err(e) = self.notification_service.create_notification(notification_request).await {
            error!("Failed to send traditional notification: {}", e);
        }

        // 2. 发送实时WebSocket通知
        let ws_message = WebSocketMessage::notification(
            json!({
                "type": notification_type,
                "title": title,
                "content": content,
                "data": data,
                "timestamp": Utc::now()
            }),
            user_id.to_string(),
        );

        if let Err(e) = self.websocket_service.send_to_user(user_id, ws_message).await {
            error!("Failed to send WebSocket notification: {}", e);
        }

        Ok(())
    }

    /// 文章相关实时事件
    pub async fn notify_article_published(&self, article: &Article) -> Result<()> {
        info!("Broadcasting article published: {}", article.id);

        // 通知作者粉丝
        let followers = self.get_user_followers(&article.author_id).await?;
        
        for follower_id in followers {
            self.send_notification(
                &follower_id,
                "new_article",
                "有新文章发布",
                &format!("{} 发布了新文章：{}", article.author_id, article.title),
                Some(json!({
                    "article_id": article.id,
                    "author_id": article.author_id,
                    "title": article.title,
                    "excerpt": article.excerpt.clone().unwrap_or_default()
                })),
            ).await?;
        }

        // 广播到全局活动频道
        let broadcast_message = WebSocketMessage::broadcast(
            WebSocketMessageType::NewArticle,
            ChannelType::GlobalActivity.channel_name(""),
            json!({
                "article_id": article.id,
                "author_id": article.author_id,
                "title": article.title,
                "excerpt": article.excerpt.clone().unwrap_or_default(),
                "published_at": article.published_at
            }),
        );

        self.websocket_service
            .broadcast_to_channel(&ChannelType::GlobalActivity.channel_name(""), broadcast_message)
            .await?;

        Ok(())
    }

    /// 评论相关实时事件
    pub async fn notify_comment_created(&self, comment: &Comment, article: &Article) -> Result<()> {
        info!("Broadcasting new comment: {} on article: {}", comment.id, comment.article_id);

        // 通知文章作者
        if comment.author_id != article.author_id {
            self.send_notification(
                &article.author_id,
                "new_comment",
                "有新评论",
                &format!("您的文章《{}》收到了新评论", article.title),
                Some(json!({
                    "comment_id": comment.id,
                    "article_id": comment.article_id,
                    "commenter_id": comment.author_id,
                    "content": comment.content
                })),
            ).await?;
        }

        // 广播到文章评论频道
        let channel = ChannelType::ArticleComments.channel_name(&comment.article_id);
        let broadcast_message = WebSocketMessage::broadcast(
            WebSocketMessageType::NewComment,
            channel.clone(),
            json!({
                "comment_id": comment.id,
                "article_id": comment.article_id,
                "user_id": comment.author_id,
                "content": comment.content,
                "created_at": comment.created_at
            }),
        );

        self.websocket_service
            .broadcast_to_channel(&channel, broadcast_message)
            .await?;

        Ok(())
    }

    /// 点赞相关实时事件
    pub async fn notify_article_clapped(&self, article_id: &str, user_id: &str, clap_count: i32, total_claps: i32) -> Result<()> {
        debug!("Broadcasting article clap: {} by user: {} count: {}", article_id, user_id, clap_count);

        // 广播到文章点赞频道
        let channel = ChannelType::ArticleClaps.channel_name(article_id);
        let broadcast_message = WebSocketMessage::broadcast(
            WebSocketMessageType::NewClap,
            channel.clone(),
            json!({
                "article_id": article_id,
                "user_id": user_id,
                "clap_count": clap_count,
                "total_claps": total_claps,
                "timestamp": Utc::now()
            }),
        );

        self.websocket_service
            .broadcast_to_channel(&channel, broadcast_message)
            .await?;

        Ok(())
    }

    /// 关注相关实时事件
    pub async fn notify_user_followed(&self, follower_id: &str, followed_id: &str) -> Result<()> {
        info!("User {} followed user {}", follower_id, followed_id);

        // 通知被关注的用户
        self.send_notification(
            followed_id,
            "new_follower",
            "有新粉丝",
            "您有了新的关注者",
            Some(json!({
                "follower_id": follower_id,
                "followed_id": followed_id,
                "timestamp": Utc::now()
            })),
        ).await?;

        // 发送用户更新到个人频道
        let channel = ChannelType::UserActivity.channel_name(followed_id);
        let user_update_message = WebSocketMessage::broadcast(
            WebSocketMessageType::NewFollower,
            channel.clone(),
            json!({
                "follower_id": follower_id,
                "followed_id": followed_id,
                "action": "follow",
                "timestamp": Utc::now()
            }),
        );

        self.websocket_service
            .broadcast_to_channel(&channel, user_update_message)
            .await?;

        Ok(())
    }

    /// 订阅相关实时事件
    pub async fn notify_subscription_updated(&self, creator_id: &str, subscriber_id: &str, action: &str) -> Result<()> {
        info!("Subscription {} for creator {} by user {}", action, creator_id, subscriber_id);

        // 通知创作者
        self.send_notification(
            creator_id,
            "subscription_update",
            "订阅状态更新",
            &format!("您的订阅状态有更新：{}", action),
            Some(json!({
                "creator_id": creator_id,
                "subscriber_id": subscriber_id,
                "action": action,
                "timestamp": Utc::now()
            })),
        ).await?;

        // 广播到创作者更新频道
        let channel = ChannelType::CreatorUpdates.channel_name(creator_id);
        let update_message = WebSocketMessage::broadcast(
            WebSocketMessageType::SubscriptionUpdate,
            channel.clone(),
            json!({
                "creator_id": creator_id,
                "subscriber_id": subscriber_id,
                "action": action,
                "timestamp": Utc::now()
            }),
        );

        self.websocket_service
            .broadcast_to_channel(&channel, update_message)
            .await?;

        Ok(())
    }

    /// 支付相关实时事件
    pub async fn notify_payment_completed(&self, user_id: &str, amount: i64, currency: &str, item_type: &str, item_id: &str) -> Result<()> {
        info!("Payment completed for user {} amount {} {} for {} {}", user_id, amount, currency, item_type, item_id);

        // 通知用户支付成功
        self.send_notification(
            user_id,
            "payment_update",
            "支付成功",
            &format!("您的{}支付已成功完成", item_type),
            Some(json!({
                "user_id": user_id,
                "amount": amount,
                "currency": currency,
                "item_type": item_type,
                "item_id": item_id,
                "status": "completed",
                "timestamp": Utc::now()
            })),
        ).await?;

        Ok(())
    }

    /// 收益相关实时事件
    pub async fn notify_revenue_updated(&self, creator_id: &str, amount: i64, currency: &str, source: &str) -> Result<()> {
        info!("Revenue updated for creator {} amount {} {} from {}", creator_id, amount, currency, source);

        // 通知创作者收益更新
        self.send_notification(
            creator_id,
            "revenue_update",
            "收益更新",
            &format!("您有新的收益：{} {}", amount as f64 / 100.0, currency),
            Some(json!({
                "creator_id": creator_id,
                "amount": amount,
                "currency": currency,
                "source": source,
                "timestamp": Utc::now()
            })),
        ).await?;

        // 广播到创作者收益频道
        let channel = ChannelType::CreatorRevenue.channel_name(creator_id);
        let revenue_message = WebSocketMessage::broadcast(
            WebSocketMessageType::RevenueUpdate,
            channel.clone(),
            json!({
                "creator_id": creator_id,
                "amount": amount,
                "currency": currency,
                "source": source,
                "timestamp": Utc::now()
            }),
        );

        self.websocket_service
            .broadcast_to_channel(&channel, revenue_message)
            .await?;

        Ok(())
    }

    /// 系统公告
    pub async fn broadcast_system_announcement(&self, title: &str, content: &str, level: &str) -> Result<()> {
        info!("Broadcasting system announcement: {}", title);

        let announcement_message = WebSocketMessage::broadcast(
            WebSocketMessageType::SystemAnnouncement,
            ChannelType::SystemUpdates.channel_name(""),
            json!({
                "title": title,
                "content": content,
                "level": level,
                "timestamp": Utc::now()
            }),
        );

        self.websocket_service
            .broadcast_to_channel(&ChannelType::SystemUpdates.channel_name(""), announcement_message)
            .await?;

        Ok(())
    }

    /// 维护通知
    pub async fn broadcast_maintenance_notice(&self, start_time: DateTime<Utc>, duration_minutes: i32, message: &str) -> Result<()> {
        info!("Broadcasting maintenance notice starting at {}", start_time);

        let maintenance_message = WebSocketMessage::broadcast(
            WebSocketMessageType::MaintenanceNotice,
            ChannelType::SystemUpdates.channel_name(""),
            json!({
                "start_time": start_time,
                "duration_minutes": duration_minutes,
                "message": message,
                "timestamp": Utc::now()
            }),
        );

        self.websocket_service
            .broadcast_to_channel(&ChannelType::SystemUpdates.channel_name(""), maintenance_message)
            .await?;

        Ok(())
    }

    /// 获取用户粉丝列表 (简化实现)
    async fn get_user_followers(&self, user_id: &str) -> Result<Vec<String>> {
        // TODO: 从数据库获取用户粉丝列表
        // 这里返回空列表作为占位符
        debug!("Getting followers for user: {}", user_id);
        Ok(Vec::new())
    }

    /// 映射通知类型
    fn map_notification_type(&self, notification_type: &str) -> NotificationType {
        use crate::models::notification::NotificationType;
        
        match notification_type {
            "new_article" => NotificationType::ArticlePublished,
            "new_comment" => NotificationType::Comment,
            "new_follower" => NotificationType::Follow,
            "article_clap" => NotificationType::Clap,
            _ => NotificationType::Mention, // 使用 Mention 作为默认类型
        }
    }

    /// 批量发送通知
    pub async fn send_batch_notifications(
        &self,
        notifications: Vec<(String, String, String, String, Option<serde_json::Value>)>
    ) -> Result<()> {
        info!("Sending batch notifications: {} items", notifications.len());

        for (user_id, notification_type, title, content, data) in notifications {
            if let Err(e) = self.send_notification(&user_id, &notification_type, &title, &content, data).await {
                error!("Failed to send notification to {}: {}", user_id, e);
            }
        }

        Ok(())
    }

    /// 获取实时统计
    pub async fn get_realtime_stats(&self) -> Result<serde_json::Value> {
        let ws_stats = self.websocket_service.get_stats().await;
        
        Ok(json!({
            "websocket": {
                "total_connections": ws_stats.total_connections,
                "active_users": ws_stats.active_users,
                "channels": ws_stats.channels.len(),
                "messages_sent_24h": ws_stats.messages_sent_24h
            },
            "timestamp": Utc::now()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_realtime_service_creation() {
        // 基本的编译测试
        // 完整测试需要实际的服务实例
    }
}