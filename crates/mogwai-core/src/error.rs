//! Errors

/// An error type.
#[derive(Debug)]
pub struct Error(anyhow::Error);

impl<T: std::error::Error + Send + Sync + 'static> From<T> for Error {
    fn from(e: T) -> Self {
        Error(anyhow::Error::new(e))
    }
}
