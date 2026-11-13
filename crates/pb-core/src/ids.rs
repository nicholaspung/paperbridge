use crate::error::{Error, Result};
use std::fmt;

/// A document or folder ID that has been checked to be a UUID, and is
/// therefore safe to place in a URL path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocId(String);

impl DocId {
    pub fn parse(s: &str) -> Result<Self> {
        let b = s.as_bytes();
        let ok = b.len() == 36
            && b.iter().enumerate().all(|(i, c)| match i {
                8 | 13 | 18 | 23 => *c == b'-',
                _ => c.is_ascii_hexdigit(),
            });
        if ok {
            Ok(DocId(s.to_ascii_lowercase()))
        } else {
            Err(Error::InvalidDocumentId(s.chars().take(64).collect()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DocId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_uuid() {
        let id = DocId::parse("0A1b2c3d-0000-4111-8222-abcdefABCDEF").unwrap();
        assert_eq!(id.as_str(), "0a1b2c3d-0000-4111-8222-abcdefabcdef");
    }

    #[test]
    fn rejects_everything_else() {
        for bad in [
            "",
            "..",
            "../../etc/passwd",
            "0a1b2c3d-0000-4111-8222-abcdefabcde",
            "0a1b2c3d-0000-4111-8222-abcdefabcdef/",
            "0a1b2c3d-0000-4111-8222-abcdefabcdeg",
            "0a1b2c3d00004111-8222-abcdefabcdef000",
            "0a1b2c3d-0000-4111-8222-abcdefabcdef?x=1",
            "0a1b2c3d-0000-4111-8222-abcdéfabcdef",
        ] {
            assert!(DocId::parse(bad).is_err(), "{bad:?} should be rejected");
        }
    }
}
