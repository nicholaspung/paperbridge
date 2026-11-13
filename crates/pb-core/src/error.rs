use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not a valid document ID: {0:?}")]
    InvalidDocumentId(String),
    #[error("no reMarkable USB network interface found")]
    NoUsbInterface,
    #[error("invalid request path: {0:?}")]
    InvalidPath(String),
    #[error("timed out after {0:?} while {1}")]
    Timeout(Duration, &'static str),
    #[error("response larger than the {0} byte limit")]
    TooLarge(u64),
    #[error("unexpected response from the tablet: {0}")]
    UnsupportedResponse(String),
    #[error("device request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
