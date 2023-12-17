use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Only api v2 is supported")]
    UnsupportedRegistry,

    #[error("Found invalid header value for header '{0}'")]
    InvalidHeaderValue(String),

    #[error("There was an error during the request: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("Received error from api: '{0}'")]
    RegistryError(String),

    #[error("The given blob can't be converted to the provided struct type")]
    InvalidBlobType,

    #[error("The response object is missing the 'mediaType' field")]
    MissingMediaType,

    #[error("The response didn't contain the 'Docker-Content-Digest' header")]
    MissingDigest,

    #[error("The manifest list didn't contain any manifests")]
    EmptyManifestList,
}