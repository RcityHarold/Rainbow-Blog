use crate::{error::Result, services::Database, config::Config};
use std::sync::Arc;

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
}