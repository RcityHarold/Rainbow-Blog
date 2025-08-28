use crate::{error::Result, services::Database};
use std::sync::Arc;

#[derive(Clone)]
pub struct PublicationService {
    db: Arc<Database>,
}

impl PublicationService {
    pub async fn new(db: Arc<Database>) -> Result<Self> {
        Ok(Self { db })
    }
}