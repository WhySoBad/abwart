use std::sync::Arc;
use chrono::{DateTime, Utc};
use crate::api::layer::Layer;
use crate::api::repository::Repository;
use crate::api::ApiManifest;
use crate::api::{get_request_client, DistributionConfig, MANIFEST_CONTENT_TYPE};
use futures::future::try_join_all;
use serde::Deserialize;
use crate::api::error::ApiError;
use crate::api::request::handle_response;

#[derive(Debug, Clone)]
pub struct Manifest {
    pub repository: Arc<Repository>,
    pub manifest_config: Layer,
    pub schema_version: u32,
    pub media_type: String,
    pub layers: Vec<Layer>,
    pub digest: String,
}

impl Manifest {
    pub fn new(
        schema_version: u32,
        media_type: String,
        layers: Vec<Layer>,
        repository: Arc<Repository>,
        manifest_config: Layer,
        digest: String,
    ) -> Self {
        Self {
            schema_version,
            digest,
            media_type,
            layers,
            repository,
            manifest_config,
        }
    }

    /// Get the config blob for the manifest
    pub async fn get_config(&self) -> Result<ManifestConfig, ApiError> {
        let blob = self
            .repository
            .pull_blob(
                self.manifest_config.digest.as_str(),
                self.manifest_config.media_type.as_str(),
            )
            .await?;
        serde_json::from_value::<ManifestConfig>(blob).map_err(|_| ApiError::InvalidBlobType)
    }
}

#[derive(Debug)]
pub struct ManifestList {
    pub repository: Arc<Repository>,
    config: Arc<DistributionConfig>,
    pub digest: String,
    pub schema_version: u32,
    pub media_type: String,
    pub manifests: Vec<Layer>,
}

impl ManifestList {
    pub fn new(
        schema_version: u32,
        media_type: String,
        manifests: Vec<Layer>,
        repository: Arc<Repository>,
        digest: String,
        config: Arc<DistributionConfig>,
    ) -> Self {
        Self {
            schema_version,
            media_type,
            manifests,
            repository,
            digest,
            config,
        }
    }

    /// Get a specific manifest from the manifest list by it's digest
    pub async fn get_manifest(&self, digest: String) -> Result<Manifest, ApiError> {
        let content_type = self
            .manifests
            .iter()
            .find(|m| m.digest == digest)
            .map(|l| l.media_type.clone())
            .unwrap_or(String::from(MANIFEST_CONTENT_TYPE));
        let client = get_request_client(format!("{content_type}").as_str())?;
        let mut resp = client
            .get(self.config.url(format!("/v2/{}/manifests/{digest}", self.repository.name).as_str()))
            .send()
            .await?;
        resp = handle_response(resp).await?;

        let manifest = resp.json::<ApiManifest>().await?;
        Ok(Manifest::new(
            manifest.schema_version,
            manifest.media_type,
            manifest.layers,
            self.repository.clone(),
            manifest.config,
            digest,
        ))
    }

    /// Get all manifests of the manifest list in parallel
    pub async fn get_all_manifests(&self) -> Result<Vec<Manifest>, ApiError> {
        let mut requests = Vec::new();
        for manifest in &self.manifests {
            requests.push(self.get_manifest(manifest.digest.clone()));
        }
        let manifests = try_join_all(requests).await?;
        Ok(manifests)
    }
}

#[derive(Debug)]
pub enum ManifestResponse {
    ManifestList(ManifestList),
    Manifest(Manifest),
}

/// **Note:** <br>
/// Only added minimal amount of fields which are needed for the sake of this application to
/// the config
///
/// When more fields are needed one can simply add them but one has to make sure it can be deserialized
/// by serde
#[derive(Deserialize, Debug)]
pub struct ManifestConfig {
    pub created: DateTime<Utc>,
}
