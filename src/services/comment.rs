use crate::{error::Result, services::Database};
use std::sync::Arc;

#[derive(Clone)]
pub struct CommentService {
    db: Arc<Database>,
}

impl CommentService {
    pub async fn new(db: Arc<Database>) -> Result<Self> {
        Ok(Self { db })
    }
}