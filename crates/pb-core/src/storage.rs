//! Local SQLite store: the cached library listing and small settings.

use crate::ids::DocId;
use crate::listing::{DocKind, DocumentRecord};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;

pub struct Store {
    conn: Mutex<Connection>,
}

type Result<T> = rusqlite::Result<T>;

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        Self::init(Connection::open(path)?)
    }

    pub fn in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS documents (
                 id TEXT PRIMARY KEY, parent TEXT, title TEXT NOT NULL, kind TEXT NOT NULL,
                 modified TEXT, bookmarked INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS kv (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )?;
        Ok(Store {
            conn: Mutex::new(conn),
        })
    }

    /// Replaces the cached library in one transaction.
    pub fn replace_library(&self, records: &[DocumentRecord]) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM documents", [])?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO documents (id, parent, title, kind, modified, bookmarked)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for r in records {
                stmt.execute(params![
                    r.id.as_str(),
                    r.parent.as_ref().map(DocId::as_str),
                    r.title,
                    r.kind.as_str(),
                    r.modified,
                    r.bookmarked
                ])?;
            }
        }
        tx.commit()
    }

    pub fn load_library(&self) -> Result<Vec<DocumentRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, parent, title, kind, modified, bookmarked FROM documents")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, bool>(5)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, parent, title, kind, modified, bookmarked) = row?;
            // Rows that no longer validate are dropped rather than trusted.
            let Ok(id) = DocId::parse(&id) else { continue };
            let parent = match parent {
                None => None,
                Some(p) => match DocId::parse(&p) {
                    Ok(p) => Some(p),
                    Err(_) => continue,
                },
            };
            out.push(DocumentRecord {
                id,
                parent,
                title,
                kind: DocKind::parse(&kind),
                modified,
                bookmarked,
            });
        }
        Ok(out)
    }

    pub fn get(&self, key: &str) -> Result<Option<String>> {
        self.conn
            .lock()
            .unwrap()
            .query_row("SELECT value FROM kv WHERE key = ?1", [key], |r| r.get(0))
            .optional()
    }

    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO kv (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(id: &str, parent: Option<&str>, title: &str, kind: DocKind) -> DocumentRecord {
        DocumentRecord {
            id: DocId::parse(id).unwrap(),
            parent: parent.map(|p| DocId::parse(p).unwrap()),
            title: title.into(),
            kind,
            modified: Some("2000-01-01T00:00:00.1Z".into()),
            bookmarked: true,
        }
    }

    #[test]
    fn library_round_trip_and_replace() {
        let a = "0a1b2c3d-0000-4111-8222-abcdefabcdef";
        let b = "1a1b2c3d-0000-4111-8222-abcdefabcdef";
        let s = Store::in_memory().unwrap();
        let first = vec![
            rec(a, None, "Folder '); DROP TABLE documents;--", DocKind::Folder),
            rec(b, Some(a), "Ünïcode", DocKind::Epub),
        ];
        s.replace_library(&first).unwrap();
        let mut got = s.load_library().unwrap();
        got.sort_by(|x, y| x.id.cmp(&y.id));
        assert_eq!(got, first);

        s.replace_library(&first[..1]).unwrap();
        assert_eq!(s.load_library().unwrap().len(), 1);
    }

    #[test]
    fn settings() {
        let s = Store::in_memory().unwrap();
        assert_eq!(s.get("first_run_done").unwrap(), None);
        s.set("first_run_done", "1").unwrap();
        s.set("first_run_done", "2").unwrap();
        assert_eq!(s.get("first_run_done").unwrap().as_deref(), Some("2"));
    }
}
