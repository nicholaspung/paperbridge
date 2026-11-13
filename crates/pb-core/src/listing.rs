//! Turns the tablet's listing JSON into `DocumentRecord`s (plan §4.1, §6).
//!
//! Tolerant by design: every field is optional, unknown values map to
//! `Unknown`, and one bad item never fails the whole listing.

use crate::error::{Error, Result};
use crate::ids::DocId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum DocKind {
    Folder,
    Pdf,
    Epub,
    Notebook,
    Unknown,
}

impl DocKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DocKind::Folder => "folder",
            DocKind::Pdf => "pdf",
            DocKind::Epub => "epub",
            DocKind::Notebook => "notebook",
            DocKind::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "folder" => DocKind::Folder,
            "pdf" => DocKind::Pdf,
            "epub" => DocKind::Epub,
            "notebook" => DocKind::Notebook,
            _ => DocKind::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DocumentRecord {
    #[ts(type = "string")]
    pub id: DocId,
    /// `None` for items in the root folder.
    #[ts(type = "string | null")]
    pub parent: Option<DocId>,
    pub title: String,
    pub kind: DocKind,
    /// RFC 3339 timestamp as sent by the tablet.
    pub modified: Option<String>,
    pub bookmarked: bool,
}

impl Serialize for DocId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Listing {
    pub records: Vec<DocumentRecord>,
    /// Items dropped because they had no valid ID or were not objects.
    pub skipped: usize,
}

/// The tablet labels its JSON ISO-8859-1. Try UTF-8 first; fall back to
/// Latin-1, where every byte is a code point.
fn decode(body: &[u8]) -> String {
    match std::str::from_utf8(body) {
        Ok(s) => s.to_string(),
        Err(_) => body.iter().map(|&b| b as char).collect(),
    }
}

pub fn normalize(body: &[u8]) -> Result<Listing> {
    let value: Value = serde_json::from_str(&decode(body))
        .map_err(|e| Error::UnsupportedResponse(format!("listing is not JSON: {e}")))?;
    let Value::Array(items) = value else {
        return Err(Error::UnsupportedResponse("listing is not a JSON array".into()));
    };
    let mut out = Listing::default();
    for item in &items {
        let text = |k: &str| item.get(k).and_then(Value::as_str).unwrap_or("");
        let Ok(id) = DocId::parse(text("ID")) else {
            out.skipped += 1;
            continue;
        };
        let parent = match text("Parent") {
            "" => None,
            p => match DocId::parse(p) {
                Ok(p) => Some(p),
                // "trash" or anything unexpected: not part of the library.
                Err(_) => {
                    out.skipped += 1;
                    continue;
                }
            },
        };
        let kind = match (text("Type"), text("fileType")) {
            ("CollectionType", _) => DocKind::Folder,
            ("DocumentType", "pdf") => DocKind::Pdf,
            ("DocumentType", "epub") => DocKind::Epub,
            ("DocumentType", "notebook") => DocKind::Notebook,
            _ => DocKind::Unknown,
        };
        let title = [text("VissibleName"), text("VisibleName")]
            .into_iter()
            .find(|t| !t.trim().is_empty())
            .unwrap_or("(untitled)")
            .to_string();
        out.records.push(DocumentRecord {
            id,
            parent,
            title,
            kind,
            modified: Some(text("ModifiedClient").to_string()).filter(|s| !s.is_empty()),
            bookmarked: item.get("Bookmarked").and_then(Value::as_bool).unwrap_or(false),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "0a1b2c3d-0000-4111-8222-abcdefabcdef";
    const B: &str = "1a1b2c3d-0000-4111-8222-abcdefabcdef";

    #[test]
    fn real_fixtures_parse_completely() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/3.29.1.0");
        let mut total = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let l = normalize(&std::fs::read(entry.unwrap().path()).unwrap()).unwrap();
            assert_eq!(l.skipped, 0);
            assert!(l.records.iter().all(|r| r.kind != DocKind::Unknown));
            assert!(l.records.iter().all(|r| r.modified.is_some()));
            total += l.records.len();
        }
        assert_eq!(total, 72);
    }

    #[test]
    fn tolerant_of_missing_and_odd_fields() {
        let body = format!(
            r#"[
            {{"ID":"{A}","Type":"CollectionType","VisibleName":"Only new spelling"}},
            {{"ID":"{B}","Parent":"{A}","Type":"DocumentType","fileType":"djvu","Bookmarked":"yes"}},
            {{"ID":"not-a-uuid","VissibleName":"x"}},
            {{"ID":"2a1b2c3d-0000-4111-8222-abcdefabcdef","Parent":"trash"}},
            42
        ]"#
        );
        let l = normalize(body.as_bytes()).unwrap();
        assert_eq!(l.skipped, 3);
        assert_eq!(l.records.len(), 2);
        assert_eq!(l.records[0].kind, DocKind::Folder);
        assert_eq!(l.records[0].title, "Only new spelling");
        assert_eq!(l.records[0].parent, None);
        assert_eq!(l.records[1].kind, DocKind::Unknown);
        assert_eq!(l.records[1].title, "(untitled)");
        assert_eq!(l.records[1].parent.as_ref().unwrap().as_str(), A);
        assert!(!l.records[1].bookmarked);
    }

    #[test]
    fn rejects_non_listings() {
        for bad in [&b"<html>"[..], b"{}", b"", b"[1,2", b"null"] {
            assert!(matches!(normalize(bad), Err(Error::UnsupportedResponse(_))));
        }
    }

    #[test]
    fn latin1_titles_survive() {
        let mut body = format!(r#"[{{"ID":"{A}","VissibleName":"Caf"#).into_bytes();
        body.push(0xE9); // é in ISO-8859-1, invalid as UTF-8
        body.extend_from_slice(br#""}]"#);
        assert_eq!(normalize(&body).unwrap().records[0].title, "Café");
    }
}
