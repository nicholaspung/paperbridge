//! PaperBridge core: device transport and host detection. No GUI dependencies.

pub mod error;
pub mod ids;
pub mod platform;
pub mod transport;

pub use error::Error;
pub use ids::DocId;

/// Where the connection to the tablet stands (plan §4.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    NoUsbInterface,
    UsbInterfaceUp,
    Connected,
    LocalNetworkPermissionDenied,
    WebInterfaceUnreachable,
    UnsupportedResponse,
    TransportError(String),
}
