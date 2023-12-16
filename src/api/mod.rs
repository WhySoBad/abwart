use crate::api::layer::Layer;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT};
use reqwest::{Client, ClientBuilder};
use serde::Deserialize;
use crate::api::error::ApiError;

pub mod distribution;
pub mod layer;
pub mod manifest;
pub mod repository;
pub mod error;
pub mod tag;
mod request;

pub const INDEX_CONTENT_TYPE: &str = "application/vnd.oci.image.index.v1+json,application/vnd.docker.distribution.manifest.list.v2+json";
pub const MANIFEST_CONTENT_TYPE: &str = "application/vnd.oci.image.manifest.v1+json,application/vnd.docker.distribution.manifest.v2+json";

#[derive(Deserialize, Debug)]
pub struct ApiCatalog {
    pub repositories: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct ApiTags {
    pub name: String,
    pub tags: Option<Vec<String>>,
}

#[derive(Deserialize, Debug)]
pub struct ApiManifestList {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    #[serde(rename = "mediaType")]
    pub media_type: String,
    pub manifests: Vec<Layer>,
}

#[derive(Deserialize, Debug)]
pub struct ApiManifest {
    pub config: Layer,
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    #[serde(rename = "mediaType")]
    pub media_type: String,
    pub layers: Vec<Layer>,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct DistributionConfig {
    host: String,
    username: Option<String>,
    password: Option<String>,
    insecure: bool,
}

impl DistributionConfig {
    pub fn new(host: String, username: Option<String>, password: Option<String>, insecure: bool) -> Self {
        Self {
            host,
            username,
            password,
            insecure,
        }
    }

    pub fn url(&self, rest: &str) -> String {
        let protocol;
        if self.insecure {
            protocol = "http"
        } else {
            protocol = "https"
        }
        if self.username.is_some() && self.password.is_some() {
            format!(
                "{}://{}:{}@{}{}",
                protocol,
                self.username.clone().expect("username exists"),
                self.password.clone().expect("password exists"),
                self.host,
                rest
            )
        } else {
            format!("{}://{}{}", protocol, self.host, rest)
        }
    }
}

fn get_request_client(accept: &str) -> Result<Client, ApiError> {
    let mut headers = HeaderMap::new();
    headers.append(
        ACCEPT,
        HeaderValue::from_str(accept)
            .map_err(|_| ApiError::InvalidHeaderValue(String::from(accept)))?,
    );
    ClientBuilder::new()
        .default_headers(headers)
        .build()
        .map_err(|e| e.into())
}
