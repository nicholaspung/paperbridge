//! Reading `.rmdoc` archives (plan §7.3): a ZIP holding `<id>.content`,
//! `<id>.metadata`, `<id>.pagedata`, per-page strokes under `<id>/`, and for
//! imported books the original file (`<id>.epub`, `<id>.pdf`).
//!
//! Archives come from the tablet, so they are treated as untrusted: entry
//! count, sizes and compression ratio are capped, and only one named entry
//! is ever extracted, to a path chosen here.

use crate::error::{Error, Result};
use crate::fsutil::{rename_noclobber, sync_dir, TempFile};
use crate::ids::DocId;
use std::io::{Read, Write};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Limits {
    pub max_entries: usize,
    /// Largest uncompressed size of the extracted entry.
    pub max_entry: u64,
    /// Largest uncompressed : compressed ratio of the extracted entry.
    pub max_ratio: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { max_entries: 20_000, max_entry: 2 << 30, max_ratio: 100 }
    }
}

fn bad(msg: impl Into<String>) -> Error {
    Error::BadArchive(msg.into())
}

/// Extracts the document's original `<id>.<ext>` from `archive` into `dest`
/// (which must not exist), atomically. Returns the size written.
pub fn extract_original(archive: &Path, id: &DocId, ext: &str, dest: &Path, limits: &Limits) -> Result<u64> {
    let file = std::fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| bad(format!("not a valid archive: {e}")))?;
    if zip.len() > limits.max_entries {
        return Err(bad(format!("{} entries", zip.len())));
    }
    let name = format!("{id}.{ext}");
    let mut entry = zip
        .by_name(&name)
        .map_err(|_| bad(format!("the archive has no original .{ext} file")))?;
    let size = entry.size();
    let packed = entry.compressed_size().max(1);
    if size > limits.max_entry {
        return Err(Error::TooLarge(limits.max_entry));
    }
    if size / packed > limits.max_ratio {
        return Err(bad(format!("suspicious compression ratio {}:1", size / packed)));
    }

    let dir = dest.parent().ok_or_else(|| Error::InvalidPath(dest.display().to_string()))?;
    let tmp = TempFile::new_in(dir, ext);
    let mut out = std::fs::OpenOptions::new().write(true).create_new(true).open(tmp.path())?;
    // Never trust the declared size: stop one byte past it.
    let written = std::io::copy(&mut (&mut entry).take(size + 1), &mut out)?;
    if written != size {
        return Err(bad("entry size does not match its header"));
    }
    out.flush()?;
    out.sync_all()?;
    drop(out);

    let mut head = [0u8; 4];
    std::fs::File::open(tmp.path())?.read_exact(&mut head).map_err(|_| bad("empty file"))?;
    let ok = match ext {
        "epub" => head == *b"PK\x03\x04",
        "pdf" => head == *b"%PDF",
        _ => true,
    };
    if !ok {
        return Err(Error::BadExport(if ext == "epub" { "EPUB" } else { "PDF" }));
    }
    rename_noclobber(tmp.path(), dest)?;
    tmp.keep();
    sync_dir(dir);
    Ok(written)
}

/// Which pages of a document carry annotations, as 1-based page numbers in
/// the order the tablet exports them (deleted pages left out).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AnnotatedPages {
    /// Pages in the document.
    pub total: u32,
    pub pages: Vec<u32>,
}

const CONTENT_CAP: u64 = 16 << 20;

/// Page IDs in document order, from either `.content` layout: format 1 has
/// `pages: [id…]`; format 2 has `cPages.pages: [{id, idx: {value}, deleted}]`
/// ordered by `idx.value`.
fn page_ids(content: &serde_json::Value) -> Option<Vec<String>> {
    if let Some(list) = content.get("cPages").and_then(|c| c.get("pages")).and_then(|p| p.as_array()) {
        let mut pages: Vec<(String, String)> = list
            .iter()
            .filter(|p| p.get("deleted").and_then(|d| d.get("value")).and_then(|v| v.as_i64()).unwrap_or(0) == 0)
            .filter_map(|p| {
                let id = p.get("id")?.as_str()?.to_string();
                let idx = p.get("idx").and_then(|i| i.get("value")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                Some((idx, id))
            })
            .collect();
        pages.sort();
        return Some(pages.into_iter().map(|(_, id)| id).collect());
    }
    let list = content.get("pages")?.as_array()?;
    Some(list.iter().filter_map(|p| p.as_str().map(str::to_string)).collect())
}

/// Reads the page list and finds the pages with strokes (`<id>/<page>.rm`) or
/// text highlights (`<id>.highlights/<page>.json`). Reads names and the small
/// `.content` file only; strokes are not parsed, so a page whose strokes were
/// all erased may still count.
pub fn annotated_pages(archive: &Path, id: &DocId, limits: &Limits) -> Result<AnnotatedPages> {
    let file = std::fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| bad(format!("not a valid archive: {e}")))?;
    if zip.len() > limits.max_entries {
        return Err(bad(format!("{} entries", zip.len())));
    }
    let annotated: std::collections::HashSet<String> = zip
        .file_names()
        .filter_map(|n| {
            let n = n.ok()?;
            let rest = n.strip_prefix(id.as_str())?;
            let page = rest
                .strip_prefix('/')
                .and_then(|r| r.strip_suffix(".rm"))
                .or_else(|| rest.strip_prefix(".highlights/").and_then(|r| r.strip_suffix(".json")))?;
            (!page.is_empty() && !page.contains('/')).then(|| page.to_string())
        })
        .collect();
    let mut entry = zip
        .by_name(&format!("{id}.content"))
        .map_err(|_| bad("the archive has no .content file"))?;
    if entry.size() > CONTENT_CAP {
        return Err(Error::TooLarge(CONTENT_CAP));
    }
    let mut raw = Vec::new();
    (&mut entry).take(CONTENT_CAP + 1).read_to_end(&mut raw)?;
    let content: serde_json::Value = serde_json::from_slice(&raw).map_err(|e| bad(format!("bad .content: {e}")))?;
    let ids = page_ids(&content).ok_or_else(|| bad("the .content file lists no pages"))?;
    let pages = ids
        .iter()
        .enumerate()
        .filter(|(_, p)| annotated.contains(p.as_str()))
        .map(|(i, _)| i as u32 + 1)
        .collect();
    Ok(AnnotatedPages { total: ids.len() as u32, pages })
}

#[cfg(test)]
mod tests {
    use super::*;
    use zip::write::SimpleFileOptions;

    const ID: &str = "0a1b2c3d-0000-4111-8222-abcdefabcdef";

    fn archive(dir: &Path, entries: &[(&str, &[u8])]) -> std::path::PathBuf {
        let path = dir.join("a.rmdoc");
        let mut w = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        for (name, data) in entries {
            w.start_file(*name, SimpleFileOptions::default()).unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap();
        path
    }

    fn tmpdir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("pb-rmdoc-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn files(dir: &Path) -> Vec<String> {
        let mut v: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        v.sort();
        v
    }

    #[test]
    fn finds_annotated_pages_in_both_content_formats() {
        let dir = tmpdir("ann");
        let id = DocId::parse(ID).unwrap();
        let v1 = r#"{"formatVersion":1,"pages":["p-a","p-b","p-c","p-d"]}"#;
        let a = archive(&dir, &[
            (&format!("{ID}.content"), v1.as_bytes()),
            (&format!("{ID}/p-b.rm"), b"strokes"),
            (&format!("{ID}.highlights/p-d.json"), b"{}"),
            (&format!("{ID}/p-z.rm"), b"a page not in the list"),
            (&format!("{ID}/p-a-metadata.json"), b"{}"),
        ]);
        assert_eq!(annotated_pages(&a, &id, &Limits::default()).unwrap(), AnnotatedPages { total: 4, pages: vec![2, 4] });

        // Format 2: order by idx, deleted pages do not count.
        let v2 = r#"{"formatVersion":2,"cPages":{"pages":[
            {"id":"p-3","idx":{"value":"bc"}},
            {"id":"p-1","idx":{"value":"ba"}},
            {"id":"p-x","idx":{"value":"bb"},"deleted":{"value":1}},
            {"id":"p-2","idx":{"value":"bb"}}]}}"#;
        let a = archive(&dir, &[(&format!("{ID}.content"), v2.as_bytes()), (&format!("{ID}/p-3.rm"), b"s"), (&format!("{ID}/p-x.rm"), b"s")]);
        assert_eq!(annotated_pages(&a, &id, &Limits::default()).unwrap(), AnnotatedPages { total: 3, pages: vec![3] });

        let a = archive(&dir, &[(&format!("{ID}.content"), b"{}")]);
        assert!(matches!(annotated_pages(&a, &id, &Limits::default()), Err(Error::BadArchive(_))));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn extracts_the_original_epub() {
        let dir = tmpdir("ok");
        let epub = b"PK\x03\x04 pretend epub";
        let a = archive(&dir, &[(&format!("{ID}.content"), b"{}"), (&format!("{ID}.epub"), epub), (&format!("{ID}.pdf"), b"%PDF-1.4")]);
        let id = DocId::parse(ID).unwrap();
        let dest = dir.join("Book.epub");
        assert_eq!(extract_original(&a, &id, "epub", &dest, &Limits::default()).unwrap(), epub.len() as u64);
        assert_eq!(std::fs::read(&dest).unwrap(), epub);
        assert_eq!(files(&dir), ["Book.epub", "a.rmdoc"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn refuses_missing_wrong_and_bomb_entries() {
        let dir = tmpdir("bad");
        let id = DocId::parse(ID).unwrap();
        let dest = dir.join("Book.epub");

        let a = archive(&dir, &[(&format!("{ID}.content"), b"{}")]);
        assert!(matches!(extract_original(&a, &id, "epub", &dest, &Limits::default()), Err(Error::BadArchive(_))));

        // A path-like name for another document is not the entry we asked for.
        let a = archive(&dir, &[(&format!("../{ID}.epub"), b"PK\x03\x04")]);
        assert!(matches!(extract_original(&a, &id, "epub", &dest, &Limits::default()), Err(Error::BadArchive(_))));

        let a = archive(&dir, &[(&format!("{ID}.epub"), b"<html>not a zip</html>")]);
        assert!(matches!(extract_original(&a, &id, "epub", &dest, &Limits::default()), Err(Error::BadExport(_))));

        let zeros = vec![0u8; 1 << 20];
        let a = archive(&dir, &[(&format!("{ID}.epub"), &zeros)]);
        assert!(matches!(extract_original(&a, &id, "epub", &dest, &Limits::default()), Err(Error::BadArchive(_))));

        let a = archive(&dir, &[(&format!("{ID}.epub"), b"PK\x03\x04 data")]);
        let small = Limits { max_entry: 4, ..Limits::default() };
        assert!(matches!(extract_original(&a, &id, "epub", &dest, &small), Err(Error::TooLarge(4))));

        std::fs::write(&a, b"garbage").unwrap();
        assert!(matches!(extract_original(&a, &id, "epub", &dest, &Limits::default()), Err(Error::BadArchive(_))));

        assert_eq!(files(&dir), ["a.rmdoc"], "nothing else may be left behind");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
