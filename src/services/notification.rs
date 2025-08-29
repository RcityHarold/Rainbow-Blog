use crate::{
    error::Result, 
    services::Database, 
    config::Config,
    models::notification::*,
};
use std::sync::Arc;
use uuid::Uuid;
use chrono::Utc;

#[derive(Clone)]
pub struct NotificationService {
    db: Arc<Database>,
    config: Config,
}

impl NotificationService {
    pub async fn new(db: Arc<Database>, config: &Config) -> Result<Self> {
        Ok(Self { 
            db,
            config: config.clone(),
        })
    }

    pub async fn create_notification(&self, request: CreateNotificationRequest) -> Result<Notification> {
        let notification = Notification {
            id: Uuid::new_v4().to_string(),
            recipient_id: request.recipient_id,
            notification_type: format!("{:?}", request.notification_type),
            title: request.title,
            message: request.message,
            data: request.data,
            is_read: false,
            read_at: None,
            created_at: Utc::now(),
        };

        let created: Notification = self.db.create("notification", notification).await?;
        Ok(created)
    }
}