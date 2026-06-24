pub type ServerResult<T> = Result<T, ServerError>;

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("Error reading environment variable: {0}")]
    Env(#[from] std::env::VarError),
    #[error("Parse int error: {0}")]
    ParseInt(#[from] std::num::ParseIntError),
}
