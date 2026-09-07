//! The `Error` type returned by fallible wrapper operations.

/// Coarse classification of a wrapper error. Kept small so plugins and
/// telemetry can react to error kinds without exhausting onto every cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WrapperErrorKind {
    Config,
    Database,
    Network,
    Vm,
    Auth,
    Proxy,
    NotFound,
    InvalidInput,
    AlreadyExists,
    Plugin,
    Timeout,
    Unauthorized,
    Internal,
}

/// The primary error type of the wrapper.
///
/// Each variant carries an optional source chain and a machine readable kind
/// so that callers (and telemetry) can react without string matching.
#[derive(Debug, thiserror::Error)]
#[allow(clippy::large_enum_variant)]
pub enum Error {
    #[error("cli/config error: {0}")]
    Config(String),

    #[error("database error: {0}")]
    Database(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("vm error: {0}")]
    Vm(String),

    #[error("auth error: {0}")]
    Auth(String),

    #[error("proxy error: {0}")]
    Proxy(String),

    #[error("plugin error: {0}")]
    Plugin(String),

    #[error("{0} was not found")]
    NotFound(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("{0} already exists")]
    AlreadyExists(String),

    #[error("operation timed out: {0}")]
    Timeout(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("internal error: {0}")]
    Internal(String),
}

impl Error {
    /// Return the coarse classification of this error.
    pub fn kind(&self) -> WrapperErrorKind {
        match self {
            Error::Config(_) => WrapperErrorKind::Config,
            Error::Database(_) => WrapperErrorKind::Database,
            Error::Network(_) => WrapperErrorKind::Network,
            Error::Vm(_) => WrapperErrorKind::Vm,
            Error::Auth(_) => WrapperErrorKind::Auth,
            Error::Proxy(_) => WrapperErrorKind::Proxy,
            Error::Plugin(_) => WrapperErrorKind::Plugin,
            Error::NotFound(_) => WrapperErrorKind::NotFound,
            Error::InvalidInput(_) => WrapperErrorKind::InvalidInput,
            Error::AlreadyExists(_) => WrapperErrorKind::AlreadyExists,
            Error::Timeout(_) => WrapperErrorKind::Timeout,
            Error::Unauthorized(_) => WrapperErrorKind::Unauthorized,
            Error::Io(_) => WrapperErrorKind::Internal,
            Error::Json(_) => WrapperErrorKind::Internal,
            Error::Internal(_) => WrapperErrorKind::Internal,
        }
    }
}

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Error::Internal(err.to_string())
    }
}

impl From<WrapperErrorKind> for Error {
    fn from(kind: WrapperErrorKind) -> Self {
        Error::Internal(format!("wrapper error: {kind:?}"))
    }
}

/// Convenience alias used across the workspace.
pub type Result<T> = std::result::Result<T, Error>;