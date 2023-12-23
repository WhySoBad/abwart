use std::sync::Arc;
use crate::api::repository::Repository;
use crate::api::ApiCatalog;
use crate::api::DistributionConfig;
use crate::api::error::ApiError;
use crate::api::request::{get_follow_path, handle_response};

#[derive(Debug)]
pub struct Distribution {
    config: Arc<DistributionConfig>,
}

impl Distribution {
    pub fn new(config: Arc<DistributionConfig>) -> Self {
        Self { config }
    }

    /// Get all repositories present in the registry
    pub async fn get_repositories(&self) -> Result<Vec<Repository>, ApiError> {
        let mut images = Vec::<Repository>::new();
        let mut link = Some(self.config.url("/v2/_catalog?n=100"));

        while link.is_some() {
            let mut resp = reqwest::get(link.expect("Link exists")).await?;
            resp = handle_response(resp).await?;
            link = get_follow_path(resp.headers())?;
            if let Some(l) = link {
                link = Some(self.config.url(l.as_str()))
            }
            let body = resp.json::<ApiCatalog>().await?;
            images.append(
                &mut body
                    .repositories
                    .into_iter()
                    .map(|repo| Repository::new(repo, self.config.clone()))
                    .collect::<Vec<_>>(),
            );
        }
        Ok(images)
    }
}
