use crate::{error::Result, services::Database};
use std::sync::Arc;

#[derive(Clone)]
pub struct TagService {
    db: Arc<Database>,
}

impl TagService {
    pub async fn new(db: Arc<Database>) -> Result<Self> {
        Ok(Self { db })
    }
}