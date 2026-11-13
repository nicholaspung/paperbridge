//! PaperBridge core: device transport and host detection. No GUI dependencies.

#[cfg(all(feature = "mock-device", not(debug_assertions)))]
compile_error!("the `mock-device` feature must never be enabled in a release build");

pub mod device;
pub mod error;
pub mod ids;
pub mod listing;
pub mod platform;
pub mod privacy;
pub mod storage;
pub mod transport;

pub use error::Error;
pub use ids::DocId;

use serde::Serialize;
use ts_rs::TS;

/// Where the connection to the tablet stands (plan §4.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum ConnectionState {
    /// No reMarkable USB network interface. Nothing is sent in this state.
    NoUsbInterface,
    /// Interface found; not probed yet.
    UsbInterfaceUp,
    Connected,
    /// macOS: interface up but the OS blocked the connection.
    LocalNetworkPermissionDenied,
    /// Port 80 refused: the USB web interface setting is off.
    WebInterfaceOff,
    /// Port 80 accepts connections but never answers: the tablet's web
    /// server is stuck and the setting must be toggled.
    WebInterfaceHung,
    /// Answered, but not with a listing we understand.
    UnsupportedResponse { detail: String },
    TransportError { detail: String },
}
