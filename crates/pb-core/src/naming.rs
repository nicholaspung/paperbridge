//! File names derived from tablet titles (plan §7.3).
//!
//! The result is safe on Linux, macOS and exFAT/FAT drives: NFC, no control
//! characters, none of `/ \ : * ? " < > |`, no trailing dots or spaces, no
//! Windows reserved names, no leading dot, and at most 200 UTF-8 bytes.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

const MAX_BYTES: usize = 200;
const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Turns a title into a file name with the given extension (without dot).
pub fn safe_file_name(title: &str, ext: &str) -> String {
    let mut stem: String = title
        .nfc()
        .filter(|c| !c.is_control())
        .map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c })
        .collect();
    // A trailing ".pdf" in the title would double up with the extension.
    let dotted = format!(".{ext}");
    if stem.len() > dotted.len() && stem.to_lowercase().ends_with(&dotted.to_lowercase()) {
        stem.truncate(stem.len() - dotted.len());
    }
    let mut stem = stem.trim().trim_end_matches(['.', ' ']).to_string();
    if stem.starts_with('.') {
        stem.replace_range(..1, "_");
    }
    let base = stem.split('.').next().unwrap_or("").trim().to_uppercase();
    if RESERVED.contains(&base.as_str()) {
        stem.insert(0, '_');
    }
    if stem.is_empty() {
        stem = "Untitled".into();
    }
    let budget = MAX_BYTES - dotted.len();
    if stem.len() > budget {
        let mut cut = budget;
        while !stem.is_char_boundary(cut) {
            cut -= 1;
        }
        stem.truncate(cut);
        stem = stem.trim_end_matches(['.', ' ']).to_string();
    }
    format!("{stem}{dotted}")
}

/// `dir/name`, or `dir/name (2)`, `(3)`… so that the result does not collide,
/// ignoring case, with anything already in `dir` or in `taken`.
pub fn unique_path(dir: &Path, name: &str, taken: &HashSet<String>) -> std::io::Result<PathBuf> {
    let mut existing: HashSet<String> = taken.iter().map(|n| n.to_lowercase()).collect();
    match std::fs::read_dir(dir) {
        Ok(entries) => {
            for e in entries.flatten() {
                existing.insert(e.file_name().to_string_lossy().to_lowercase());
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    for n in 1.. {
        let candidate = if n == 1 { name.to_string() } else { format!("{stem} ({n}){ext}") };
        if !existing.contains(&candidate.to_lowercase()) {
            return Ok(dir.join(candidate));
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_titles() {
        assert_eq!(safe_file_name("Notes: week 1/2", "pdf"), "Notes_ week 1_2.pdf");
        assert_eq!(safe_file_name("a\u{0}b\nc", "pdf"), "abc.pdf");
        assert_eq!(safe_file_name("  trailing...  ", "pdf"), "trailing.pdf");
        assert_eq!(safe_file_name(".hidden", "pdf"), "_hidden.pdf");
        assert_eq!(safe_file_name("CON", "pdf"), "_CON.pdf");
        assert_eq!(safe_file_name("nul.txt", "pdf"), "_nul.txt.pdf");
        assert_eq!(safe_file_name("Report.PDF", "pdf"), "Report.pdf");
        assert_eq!(safe_file_name("", "pdf"), "Untitled.pdf");
        assert_eq!(safe_file_name("...", "pdf"), "Untitled.pdf");
        assert_eq!(safe_file_name("<>?*", "pdf"), "____.pdf");
    }

    #[test]
    fn normalizes_to_nfc() {
        // "é" as e + combining acute becomes the single code point.
        assert_eq!(safe_file_name("Cafe\u{301}", "pdf"), "Caf\u{e9}.pdf");
    }

    #[test]
    fn caps_length_on_a_char_boundary() {
        let name = safe_file_name(&"é".repeat(300), "pdf");
        assert!(name.len() <= 200, "{}", name.len());
        assert!(name.ends_with(".pdf"));
        assert!(name.trim_end_matches(".pdf").chars().all(|c| c == 'é'));
    }

    #[test]
    fn unique_ignores_case() {
        let dir = std::env::temp_dir().join(format!("pb-unique-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Report.pdf"), b"x").unwrap();
        let none = HashSet::new();
        assert_eq!(unique_path(&dir, "report.PDF", &none).unwrap(), dir.join("report (2).PDF"));
        let taken: HashSet<String> = ["REPORT (2).pdf".to_string()].into();
        assert_eq!(unique_path(&dir, "Report.pdf", &taken).unwrap(), dir.join("Report (3).pdf"));
        assert_eq!(unique_path(&dir, "Other.pdf", &none).unwrap(), dir.join("Other.pdf"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
