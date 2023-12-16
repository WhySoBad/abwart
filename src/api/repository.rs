use crate::api::manifest::{Manifest, ManifestList, ManifestResponse};
use crate::api::{get_request_client, DistributionConfig, INDEX_CONTENT_TYPE, MANIFEST_CONTENT_TYPE};
use crate::api::{ApiManifest, ApiManifestList, ApiTags};
use crate::api::error::ApiError;
use crate::api::request::{get_follow_path, handle_response};
use serde_json::Value;
use crate::api::tag::Tag;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Repository<'a> {
    pub name: String,
    config: &'a DistributionConfig,
}

impl<'a> Repository<'a> {
    pub fn new(repo: String, config: &'a DistributionConfig) -> Self {
        Self { name: repo, config }
    }

    /// Get all tags on this repository
    pub async fn get_tags(&self) -> Result<Vec<String>, ApiError> {
        let mut tags = Vec::<String>::new();
        let mut link = Some(self.config.url(format!("/v2/{}/tags/list?n=100", self.name).as_str()));

        while link.is_some() {
            let mut resp = reqwest::get(link.expect("Link exists")).await?;
            resp = handle_response(resp).await?;
            link = get_follow_path(resp.headers())?;
            if let Some(l) = link {
                link = Some(self.config.url(l.as_str()))
            }
            let body = resp.json::<ApiTags>().await?;
            tags.append(&mut body.tags.unwrap_or(vec![]));
        }
        Ok(tags)
    }

    /// Get a manifest by its tag or digest <br>
    /// Depending whether the manifest is a multi-arch, docker or oci manifest a Manifest or
    /// ManifestList is returned in form of a ManifestResponse
    pub async fn get_manifest(&self, tag: &str) -> Result<ManifestResponse, ApiError> {
        let client = get_request_client(format!("{MANIFEST_CONTENT_TYPE},{INDEX_CONTENT_TYPE}").as_str())?;
        let mut resp = client
            .get(self.config.url(format!("/v2/{}/manifests/{tag}", self.name).as_str()))
            .send()
            .await?;
        resp = handle_response(resp).await?;

        let digest = resp
            .headers()
            .get("Docker-Content-Digest")
            .ok_or(ApiError::MissingDigest)?
            .to_str()
            .map_err(|_| ApiError::InvalidHeaderValue(String::from("Docker-Content-Digest")))?
            .to_string();

        let body = resp.json::<Value>().await?;

        if let Some(media_type) = body.get("mediaType") {
            if media_type == "application/vnd.docker.distribution.manifest.v2+json" {
                // we have a single-arch manifest
                let manifest = serde_json::from_value::<ApiManifest>(body)
                    .map_err(|_| ApiError::InvalidBlobType)?;
                Ok(ManifestResponse::Manifest(Manifest::new(
                    manifest.schema_version,
                    manifest.media_type,
                    manifest.layers,
                    self,
                    manifest.config,
                    digest,
                    self.config,
                )))
            } else {
                // we have a multi-arch manifest list (aka OCI index)
                let index = serde_json::from_value::<ApiManifestList>(body)
                    .map_err(|_| ApiError::InvalidBlobType)?;
                Ok(ManifestResponse::ManifestList(ManifestList::new(
                    index.schema_version,
                    index.media_type,
                    index.manifests,
                    self,
                    digest,
                    self.config,
                )))
            }
        } else {
            Err(ApiError::MissingMediaType)
        }
    }

    /// Pull a schemaless blob by it's digest from the registry
    pub async fn pull_blob(&self, digest: &str, content_type: &str) -> Result<Value, ApiError> {
        let client = get_request_client(format!("{INDEX_CONTENT_TYPE},{MANIFEST_CONTENT_TYPE},{content_type}").as_str())?;
        let mut resp = client
            .get(self.config.url(format!("/v2/{}/blobs/{digest}", self.name).as_str()))
            .send()
            .await?;
        resp = handle_response(resp).await?;

        let body = resp.json::<Value>().await?;
        Ok(body)
    }

    /// Delete a specific tag <br>
    /// **Important**: The tag delete endpoint is not implemented in all registries therefore it's safer to
    /// use the `delete_manifest(digest)` method with the digest of the tag manifest
    pub async fn delete_tag(&self, tag: &str) -> Result<(), ApiError> {
        let client = get_request_client(format!("{INDEX_CONTENT_TYPE},{MANIFEST_CONTENT_TYPE}").as_str())?;
        let resp = client
            .delete(self.config.url(format!("/v2/{}/manifests/{tag}", self.name).as_str()))
            .send()
            .await?;
        handle_response(resp).await?;
        Ok(())
    }

    /// Delete a specific manifest by it's digest from the registry
    pub async fn delete_manifest(&self, digest: &str) -> Result<(), ApiError> {
        let client = get_request_client(format!("{INDEX_CONTENT_TYPE},{MANIFEST_CONTENT_TYPE}").as_str())?;
        let resp = client
            .delete(self.config.url(format!("/v2/{}/manifests/{digest}", self.name).as_str()))
            .send()
            .await?;
        handle_response(resp).await?;
        Ok(())
    }

    /// Delete a specific blob by it's digest from the registry
    pub async fn delete_blob(&self, digest: &str) -> Result<(), ApiError> {
        let client = get_request_client(format!("{INDEX_CONTENT_TYPE},{MANIFEST_CONTENT_TYPE}").as_str())?;
        let resp = client
            .delete(self.config.url(format!("/v2/{}/blobs/{digest}", self.name).as_str()))
            .send()
            .await?;
        handle_response(resp).await?;
        Ok(())
    }

    /// Get the tags of the repository with some basic data about the tag useful
    /// for applying the deletion rules
    pub async fn get_tags_with_data(&self) -> Result<Vec<Tag>, ApiError> {
        let mut tags = Vec::<Tag>::new();
        let raw = self.get_tags().await?;
        for tag in raw {
            match self.get_manifest(&tag).await? {
                ManifestResponse::Manifest(manifest) => {
                    let size: u32 = manifest.layers.iter().map(|l| l.size).sum();
                    let config = manifest.get_config().await?;
                    tags.push(Tag::new(tag, manifest.digest, config.created, size));
                },
                ManifestResponse::ManifestList(list) => {
                    let size: u32 = list.manifests.iter().map(|m| m.size).sum();
                    let layer = list.manifests.get(0).ok_or(ApiError::EmptyManifestList)?;
                    let manifest = list.get_manifest(layer.digest.clone()).await?;
                    let config = manifest.get_config().await?;
                    tags.push(Tag::new(tag, manifest.digest, config.created, size));
                }
            }
        }
        Ok(tags)
    }
}
