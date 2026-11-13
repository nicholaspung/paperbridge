//! High-level read operations on the tablet.

use crate::error::{Error, Result};
use crate::ids::DocId;
use crate::listing::{normalize, DocKind, DocumentRecord};
use crate::transport::{DeviceTransport, Method, Session, Target, Timeouts};
use crate::ConnectionState;
use std::collections::{BTreeSet, VecDeque};

const LISTING_CAP: u64 = 10 << 20;
/// Stops a runaway or cyclic folder tree.
const MAX_FOLDERS: usize = 5000;

pub struct Device {
    transport: DeviceTransport,
}

async fn list_in(s: &mut Session<'_>, folder: Option<&DocId>) -> Result<Vec<DocumentRecord>> {
    let path = match folder {
        None => "/documents/".to_string(),
        Some(id) => format!("/documents/{id}"),
    };
    let r = s.small(Method::GET, &path, LISTING_CAP).await?;
    if r.status != 200 {
        return Err(Error::UnsupportedResponse(format!("listing returned HTTP {}", r.status)));
    }
    Ok(normalize(&r.body)?.records)
}

impl Device {
    pub fn new(target: &Target, timeouts: Timeouts) -> Result<Self> {
        Ok(Device {
            transport: DeviceTransport::new(target, timeouts)?,
        })
    }

    pub fn transport(&self) -> &DeviceTransport {
        &self.transport
    }

    pub async fn list(&self, folder: Option<&DocId>) -> Result<Vec<DocumentRecord>> {
        list_in(&mut self.transport.session().await, folder).await
    }

    /// Lists the whole library, folder by folder, holding the device queue
    /// throughout. Ends by listing the root again so that the tablet's
    /// "last listed folder" (the upload destination) is the root.
    pub async fn walk(&self) -> Result<Vec<DocumentRecord>> {
        let mut s = self.transport.session().await;
        let mut all = list_in(&mut s, None).await?;
        let mut seen: BTreeSet<DocId> = BTreeSet::new();
        let mut queue: VecDeque<DocId> = VecDeque::new();
        let mut enqueue = |records: &[DocumentRecord], queue: &mut VecDeque<DocId>| {
            for r in records {
                if r.kind == DocKind::Folder && seen.insert(r.id.clone()) {
                    queue.push_back(r.id.clone());
                }
            }
        };
        enqueue(&all, &mut queue);
        let mut folders = 0;
        while let Some(id) = queue.pop_front() {
            folders += 1;
            if folders > MAX_FOLDERS {
                return Err(Error::UnsupportedResponse("too many folders".into()));
            }
            let children = list_in(&mut s, Some(&id)).await?;
            enqueue(&children, &mut queue);
            all.extend(children);
        }
        if folders > 0 {
            list_in(&mut s, None).await?;
        }
        // The same ID must not appear twice, whatever the tablet says.
        let mut ids = BTreeSet::new();
        all.retain(|r| ids.insert(r.id.clone()));
        Ok(all)
    }
}

/// Maps a failed request to the state shown to the user.
pub fn classify(err: &Error) -> ConnectionState {
    match err {
        Error::Http(e) if e.is_connect() => {
            if cfg!(target_os = "macos") && is_host_unreachable(e) {
                ConnectionState::LocalNetworkPermissionDenied
            } else {
                ConnectionState::WebInterfaceOff
            }
        }
        Error::Timeout(..) => ConnectionState::WebInterfaceHung,
        Error::Http(e) if e.is_timeout() => ConnectionState::WebInterfaceHung,
        Error::UnsupportedResponse(d) => ConnectionState::UnsupportedResponse { detail: d.clone() },
        Error::TooLarge(_) => ConnectionState::UnsupportedResponse {
            detail: err.to_string(),
        },
        other => ConnectionState::TransportError {
            detail: other.to_string(),
        },
    }
}

fn is_host_unreachable(e: &reqwest::Error) -> bool {
    let mut src: Option<&(dyn std::error::Error + 'static)> = Some(e);
    while let Some(s) = src {
        if let Some(io) = s.downcast_ref::<std::io::Error>() {
            // EHOSTUNREACH = 65 on macOS; the expected (unconfirmed) result of a denial.
            return io.raw_os_error() == Some(65);
        }
        src = s.source();
    }
    false
}
