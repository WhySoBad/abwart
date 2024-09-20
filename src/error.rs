#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("the registry container is missing an id")]
    MissingId,

    #[error("the registry container is missing names")]
    MissingNames,

    #[error("the registry container is missing network settings")]
    MissingNetworks,

    #[error("the container for registry '{0}' does not have a network named '{1}'")]
    NoNetwork(String, String),

    #[error("the container for registry '{0}' does not have an ip address on network '{1}'")]
    NoAddress(String, String),

    #[error("the registry container '{0}' does not exist")]
    InexistentContainer(String),

    #[error("the task for registry '{0}' was not yet started")]
    TaskNotStarted(String),

    #[error("the task for registry '{0}' could not be interrupted: {1}")]
    TaskInterruptionFailed(String, String),

    #[error("the task for registry '{0}' could not be started: {1}")]
    TaskCreationFailed(String, String),

    #[error("registry api error: {0}")]
    ApiError(#[from] dkregistry::errors::Error),

    #[error("docker client error: {0}")]
    DockerError(#[from] bollard::errors::Error),

    #[error("{0}: {1}")]
    Context(String, String),

    #[error("{0}")]
    Custom(#[from] String)
}

pub trait ErrorContext {
    fn context(self, ctx: &str) -> Self;
}

impl<T> ErrorContext for Result<T, Error> {
    fn context(self, ctx: &str) -> Self {
        self.map_err(|err| Error::Context(ctx.to_string(), err.to_string()))
    }
}

impl ErrorContext for Error {
    fn context(self, ctx: &str) -> Self {
        Error::Context(ctx.to_string(), self.to_string())
    }
}