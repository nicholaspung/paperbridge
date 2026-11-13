//! Removes anything personal from captured tablet JSON.
//!
//! Rule: a string survives only if its field is on the safe list. UUIDs are
//! swapped for stable fake ones so parent/child links still line up. Every
//! other string is replaced by a note about its shape.

use pb_core::DocId;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Fields whose values are enums or timestamps, not user content.
const SAFE_FIELDS: &[&str] = &["Type", "fileType"];
/// Timestamp fields: the date and time are replaced, the fraction format is kept.
const TIME_FIELDS: &[&str] = &["ModifiedClient"];
const TITLE_FIELDS: &[&str] = &["VissibleName", "VisibleName"];
/// Special non-UUID values of ID-like fields.
const SAFE_VALUES: &[&str] = &["", "trash"];

#[derive(Default)]
pub struct Sanitizer {
    ids: HashMap<String, String>,
    titles: HashMap<String, usize>,
}

impl Sanitizer {
    fn fake_id(&mut self, real: &DocId) -> String {
        let n = self.ids.len() + 1;
        self.ids
            .entry(real.to_string())
            .or_insert_with(|| format!("00000000-0000-4000-8000-{n:012x}"))
            .clone()
    }

    fn string(&mut self, key: Option<&str>, s: &str) -> String {
        if let Ok(id) = DocId::parse(s) {
            return self.fake_id(&id);
        }
        if SAFE_VALUES.contains(&s) || key.is_some_and(|k| SAFE_FIELDS.contains(&k)) {
            return s.to_string();
        }
        if key.is_some_and(|k| TIME_FIELDS.contains(&k)) && shape(s) == "timestamp" {
            return format!("2000-01-01T00:00:00{}", &s[19..]);
        }
        if key.is_some_and(|k| TITLE_FIELDS.contains(&k)) {
            let n = self.titles.len() + 1;
            return format!("Document {}", self.titles.entry(s.to_string()).or_insert(n));
        }
        format!("<redacted {} len={}>", shape(s), s.chars().count())
    }

    pub fn value(&mut self, v: &Value) -> Value {
        self.walk(None, v)
    }

    fn walk(&mut self, key: Option<&str>, v: &Value) -> Value {
        match v {
            Value::String(s) => Value::String(self.string(key, s)),
            Value::Array(a) => Value::Array(a.iter().map(|x| self.walk(key, x)).collect()),
            Value::Object(o) => {
                let mut out = Map::new();
                for (k, x) in o {
                    out.insert(k.clone(), self.walk(Some(k), x));
                }
                Value::Object(out)
            }
            other => other.clone(),
        }
    }

    /// Replaces any UUID path segment with its fake.
    pub fn path(&mut self, path: &str) -> String {
        path.split('/')
            .map(|seg| match DocId::parse(seg) {
                Ok(id) => self.fake_id(&id),
                Err(_) => seg.to_string(),
            })
            .collect::<Vec<_>>()
            .join("/")
    }
}

fn shape(s: &str) -> &'static str {
    if s.bytes().all(|b| b.is_ascii_digit()) {
        "digits"
    } else if s.is_ascii() && s.len() >= 19 && s.as_bytes()[4] == b'-' && s.as_bytes()[10] == b'T' {
        "timestamp"
    } else {
        "text"
    }
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Field name → (JSON types seen, one example). Examples are given only for
/// safe fields and non-string values.
pub fn inventory(items: &[Value]) -> BTreeMap<String, (BTreeSet<&'static str>, String)> {
    let mut out: BTreeMap<String, (BTreeSet<&'static str>, String)> = BTreeMap::new();
    for item in items {
        let Value::Object(o) = item else { continue };
        for (k, v) in o {
            let e = out.entry(k.clone()).or_default();
            e.0.insert(type_name(v));
            if e.1.is_empty() {
                e.1 = match v {
                    Value::String(s) if SAFE_FIELDS.contains(&k.as_str()) => format!("`{s}`"),
                    Value::String(s) if TIME_FIELDS.contains(&k.as_str()) => {
                        format!("(timestamp, {} chars)", s.len())
                    }
                    Value::String(s) if DocId::parse(s).is_ok() => "UUID".into(),
                    Value::String(s) if SAFE_VALUES.contains(&s.as_str()) => format!("`\"{s}\"`"),
                    Value::String(s) => format!("({})", shape(s)),
                    Value::Bool(_) | Value::Number(_) => format!("`{v}`"),
                    _ => String::new(),
                };
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strips_titles_and_ids_but_keeps_links() {
        let a = "0a1b2c3d-0000-4111-8222-abcdefabcdef";
        let b = "1a1b2c3d-0000-4111-8222-abcdefabcdef";
        let input = json!([
            {"ID": a, "Parent": "", "Type": "CollectionType", "VissibleName": "Tax 2026",
             "Bookmarked": false, "ModifiedClient": "2026-01-02T03:04:05.18Z"},
            {"ID": b, "Parent": a, "Type": "DocumentType", "fileType": "pdf",
             "VissibleName": "Secret plan", "VisibleName": "Secret plan", "CurrentPage": 3, "tags": ["private tag"],
             "extra": {"note": "hello", "ref": a}, "lastOpened": "1700000000000"}
        ]);
        let mut s = Sanitizer::default();
        let out = s.value(&input);
        let text = out.to_string();
        for leaked in ["Tax", "Secret", "private", "hello", a, b, "1700000000000"] {
            assert!(!text.contains(leaked), "leaked {leaked}: {text}");
        }
        assert_eq!(out[1]["Parent"], out[0]["ID"]);
        assert_eq!(out[1]["VissibleName"], out[1]["VisibleName"]);
        assert_eq!(out[1]["extra"]["ref"], out[0]["ID"]);
        assert_ne!(out[0]["ID"], out[1]["ID"]);
        assert!(DocId::parse(out[0]["ID"].as_str().unwrap()).is_ok());
        assert_eq!(out[0]["ModifiedClient"], "2000-01-01T00:00:00.18Z");
        assert_eq!(out[0]["Type"], "CollectionType");
        assert_eq!(out[1]["fileType"], "pdf");
        assert_eq!(out[1]["CurrentPage"], 3);
        assert_eq!(out[0]["Parent"], "");
        assert_eq!(out[1]["lastOpened"], "<redacted digits len=13>");
    }

    #[test]
    fn path_ids_are_replaced() {
        let mut s = Sanitizer::default();
        let p = s.path("/download/0a1b2c3d-0000-4111-8222-abcdefabcdef/pdf");
        assert_eq!(p, "/download/00000000-0000-4000-8000-000000000001/pdf");
    }
}
