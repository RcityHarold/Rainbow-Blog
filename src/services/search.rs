use crate::{error::Result, services::Database};
use std::sync::Arc;

#[derive(Clone)]
pub struct SearchService {
    db: Arc<Database>,
}

impl SearchService {
    pub async fn new(db: Arc<Database>) -> Result<Self> {
        Ok(Self { db })
    }
}