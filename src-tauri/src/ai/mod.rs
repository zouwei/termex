pub mod danger;
pub mod provider;

/// AI module error types.
#[derive(Debug, thiserror::Error)]
pub enum AiModuleError {
    #[error("provider error: {0}")]
    Provider(#[from] provider::AiError),

    #[error("no default provider configured")]
    NoDefaultProvider,

    #[error("database error: {0}")]
    Database(String),
}
