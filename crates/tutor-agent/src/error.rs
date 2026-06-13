use thiserror::Error;

#[derive(Debug, Error)]
pub enum TutorError {
    #[error("harness error: {0}")]
    Harness(#[from] llm_harness_types::HarnessError),
    #[error("capability not supported: {0}")]
    UnsupportedCapability(String),
    #[error("internal: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, TutorError>;
