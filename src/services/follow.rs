use crate::{
    error::{AppError, Result},
    models::follow::*,
    models::notification::*,
    services::{Database, NotificationService},
};
use chrono::Utc;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;

#[derive(Clone)]
pub struct FollowService {
    db: Arc<Database>,
    notification_service: NotificationService,
}

impl FollowService {
    pub async fn new(db: Arc<Database>, notification_service: NotificationService) -> Result<Self> {
        Ok(Self {
            db,
            notification_service,
        })
    }

    pub async fn follow_user(&self, follower_id: &str, following_id: &str) -> Result<()> {
        debug!("User {} following user {}", follower_id, following_id);

        // 防止自己关注自己
        if follower_id == following_id {
            return Err(AppError::BadRequest("Cannot follow yourself".to_string()));
        }

        // 检查被关注用户是否存在
        let mut response = self.db.query_with_params(
            "SELECT * FROM user_profile WHERE user_id = $user_id",
            json!({
                "user_id": following_id
            })
        ).await?;
        let following_users: Vec<Value> = response.take(0)?;
        let following_user = following_users.into_iter().next();

        if following_user.is_none() {
            return Err(AppError::NotFound("User not found".to_string()));
        }

        // 检查是否已经关注
        let mut response = self.db.query_with_params(
            r#"
                SELECT * FROM follow 
                WHERE follower_id = $follower_id 
                AND following_id = $following_id
            "#,
            json!({
                "follower_id": follower_id,
                "following_id": following_id
            })
        ).await?;
        let existing: Vec<Follow> = response.take(0)?;

        if !existing.is_empty() {
            return Err(AppError::Conflict("Already following this user".to_string()));
        }

        // 创建关注关系
        let follow = Follow {
            id: Uuid::new_v4().to_string(),
            follower_id: follower_id.to_string(),
            following_id: following_id.to_string(),
            created_at: Utc::now(),
        };

        let created_follow = self.db.create("follow", follow).await?;

        // 更新用户统计
        self.update_follow_counts(follower_id, following_id).await?;

        // 发送通知
        let notification = CreateNotificationRequest {
            recipient_id: following_id.to_string(),
            notification_type: NotificationType::Follow,
            title: "New follower".to_string(),
            message: format!("Someone just followed you"),
            data: json!({
                "follower_id": follower_id,
                "follow_id": created_follow.id
            }),
        };

        if let Err(e) = self.notification_service.create_notification(notification).await {
            // 记录错误但不中断流程
            tracing::warn!("Failed to send follow notification: {}", e);
        }

        info!("User {} followed user {}", follower_id, following_id);
        Ok(())
    }

    pub async fn unfollow_user(&self, follower_id: &str, following_id: &str) -> Result<()> {
        debug!("User {} unfollowing user {}", follower_id, following_id);

        self.db.query_with_params(
            r#"
                DELETE follow 
                WHERE follower_id = $follower_id 
                AND following_id = $following_id
            "#,
            json!({
                "follower_id": follower_id,
                "following_id": following_id
            })
        ).await?;

        // 更新用户统计
        self.update_follow_counts(follower_id, following_id).await?;

        Ok(())
    }

    pub async fn get_followers(
        &self,
        user_id: &str,
        current_user_id: Option<&str>,
        page: Option<i32>,
        limit: Option<i32>,
    ) -> Result<Vec<FollowUserInfo>> {
        debug!("Getting followers for user: {}", user_id);

        let page = page.unwrap_or(1).max(1);
        let limit = limit.unwrap_or(20).min(100);
        let offset = (page - 1) * limit;

        let query = r#"
            SELECT 
                u.user_id,
                u.username,
                u.display_name,
                u.avatar_url,
                u.bio,
                u.is_verified,
                u.article_count,
                u.follower_count
            FROM follow f
            JOIN user_profile u ON f.follower_id = u.user_id
            WHERE f.following_id = $user_id
            ORDER BY f.created_at DESC
            LIMIT $limit
            START $offset
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "user_id": user_id,
            "limit": limit,
            "offset": offset
        })).await?;
        let followers: Vec<Value> = response.take(0)?;

        let mut result = Vec::new();
        for follower_data in followers {
            let mut follower_info = serde_json::from_value::<FollowUserInfo>(follower_data)?;
            
            // 获取关注状态
            if let Some(current_user) = current_user_id {
                follower_info.is_following = self
                    .is_following(current_user, &follower_info.user_id)
                    .await?;
                follower_info.is_followed_back = self
                    .is_following(&follower_info.user_id, current_user)
                    .await?;
            } else {
                follower_info.is_following = false;
                follower_info.is_followed_back = false;
            }

            result.push(follower_info);
        }

        Ok(result)
    }

    pub async fn get_following(
        &self,
        user_id: &str,
        current_user_id: Option<&str>,
        page: Option<i32>,
        limit: Option<i32>,
    ) -> Result<Vec<FollowUserInfo>> {
        debug!("Getting following for user: {}", user_id);

        let page = page.unwrap_or(1).max(1);
        let limit = limit.unwrap_or(20).min(100);
        let offset = (page - 1) * limit;

        let query = r#"
            SELECT 
                u.user_id,
                u.username,
                u.display_name,
                u.avatar_url,
                u.bio,
                u.is_verified,
                u.article_count,
                u.follower_count
            FROM follow f
            JOIN user_profile u ON f.following_id = u.user_id
            WHERE f.follower_id = $user_id
            ORDER BY f.created_at DESC
            LIMIT $limit
            START $offset
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "user_id": user_id,
            "limit": limit,
            "offset": offset
        })).await?;
        let following: Vec<Value> = response.take(0)?;

        let mut result = Vec::new();
        for following_data in following {
            let mut following_info = serde_json::from_value::<FollowUserInfo>(following_data)?;
            
            // 获取关注状态
            if let Some(current_user) = current_user_id {
                following_info.is_following = self
                    .is_following(current_user, &following_info.user_id)
                    .await?;
                following_info.is_followed_back = self
                    .is_following(&following_info.user_id, current_user)
                    .await?;
            } else {
                following_info.is_following = false;
                following_info.is_followed_back = false;
            }

            result.push(following_info);
        }

        Ok(result)
    }

    pub async fn get_follow_stats(&self, user_id: &str, current_user_id: Option<&str>) -> Result<FollowStats> {
        debug!("Getting follow stats for user: {}", user_id);

        // 获取关注者数量
        let mut response = self.db.query_with_params(
            "SELECT count() as count FROM follow WHERE following_id = $user_id",
            json!({ "user_id": user_id })
        ).await?;
        let followers_count: Vec<Value> = response.take(0)?;

        let followers_count = followers_count
            .first()
            .and_then(|v| v.get("count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        // 获取关注数量
        let mut response = self.db.query_with_params(
            "SELECT count() as count FROM follow WHERE follower_id = $user_id",
            json!({ "user_id": user_id })
        ).await?;
        let following_count: Vec<Value> = response.take(0)?;

        let following_count = following_count
            .first()
            .and_then(|v| v.get("count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let mut stats = FollowStats {
            followers_count,
            following_count,
            is_following: false,
            is_followed_by: false,
        };

        // 获取当前用户与目标用户的关系
        if let Some(current_user) = current_user_id {
            if current_user != user_id {
                stats.is_following = self.is_following(current_user, user_id).await?;
                stats.is_followed_by = self.is_following(user_id, current_user).await?;
            }
        }

        Ok(stats)
    }

    pub async fn is_following(&self, follower_id: &str, following_id: &str) -> Result<bool> {
        let mut response = self.db.query_with_params(
            r#"
                SELECT count() as count 
                FROM follow 
                WHERE follower_id = $follower_id 
                AND following_id = $following_id
            "#,
            json!({
                "follower_id": follower_id,
                "following_id": following_id
            })
        ).await?;
        let count: Vec<Value> = response.take(0)?;

        let count = count
            .first()
            .and_then(|v| v.get("count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        Ok(count > 0)
    }

    async fn update_follow_counts(&self, follower_id: &str, following_id: &str) -> Result<()> {
        // 更新关注者的 following_count
        let query1 = r#"
            LET $count = (SELECT count() FROM follow WHERE follower_id = $user_id);
            UPDATE user_profile SET following_count = $count WHERE user_id = $user_id;
        "#;

        self.db.query_with_params(query1, json!({
            "user_id": follower_id
        })).await?;

        // 更新被关注者的 follower_count
        let query2 = r#"
            LET $count = (SELECT count() FROM follow WHERE following_id = $user_id);
            UPDATE user_profile SET follower_count = $count WHERE user_id = $user_id;
        "#;

        self.db.query_with_params(query2, json!({
            "user_id": following_id
        })).await?;

        Ok(())
    }

    pub async fn get_mutual_followers(
        &self,
        user_id: &str,
        target_user_id: &str,
        limit: Option<i32>,
    ) -> Result<Vec<FollowUserInfo>> {
        debug!("Getting mutual followers between {} and {}", user_id, target_user_id);

        let limit = limit.unwrap_or(10).min(50);

        // 获取共同关注的用户
        let query = r#"
            SELECT 
                u.user_id,
                u.username,
                u.display_name,
                u.avatar_url,
                u.bio,
                u.is_verified,
                u.article_count,
                u.follower_count
            FROM user_profile u
            WHERE u.user_id IN (
                SELECT f1.following_id
                FROM follow f1
                JOIN follow f2 ON f1.following_id = f2.following_id
                WHERE f1.follower_id = $user_id
                AND f2.follower_id = $target_user_id
            )
            LIMIT $limit
        "#;

        let mut response = self.db.query_with_params(query, json!({
            "user_id": user_id,
            "target_user_id": target_user_id,
            "limit": limit
        })).await?;
        let mutual: Vec<Value> = response.take(0)?;

        let mut result = Vec::new();
        for user_data in mutual {
            let mut user_info = serde_json::from_value::<FollowUserInfo>(user_data)?;
            // 共同关注的用户，两人都关注了
            user_info.is_following = true;
            user_info.is_followed_back = self.is_following(&user_info.user_id, user_id).await?;
            result.push(user_info);
        }

        Ok(result)
    }
}