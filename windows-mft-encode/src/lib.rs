
pub mod adapter;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Windows error: {0}")]
    Windows(#[from] windows::core::Error),
}

pub type Result<T> = std::result::Result<T, Error>;