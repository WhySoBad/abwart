use crate::api::error::ApiError;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("The registry container is missing an id")]
    MissingId,

    #[error("The registry container is missing network settings")]
    MissingNetworks,

    #[error("The registry container '{0}' doesn't have a network")]
    NoNetwork(String),

    #[error("The registry container '{0}' doesn't exist")]
    InexistentContainer(String),

    #[error("The task for registry '{0}' was not yet started")]
    TaskNotStarted(String),

    #[error("The task for registry '{0}' couldn't be interrupted. Reason: {1}")]
    TaskInterruptionFailed(String, String),

    #[error("The task for registry '{0}' couldn't be started. Reason: {1}")]
    TaskCreationFailed(String, String),

    #[error("There was an api error: {0}")]
    ApiError(#[from] ApiError)
}
