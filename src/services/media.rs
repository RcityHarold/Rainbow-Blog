use crate::{error::Result, config::Config};

#[derive(Clone)]
pub struct MediaService {
    config: Config,
}

impl MediaService {
    pub async fn new(config: &Config) -> Result<Self> {
        Ok(Self { 
            config: config.clone(),
        })
    }
}