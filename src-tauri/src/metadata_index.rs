use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Clone)]
pub struct RecordingMetadata {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub size: u64,
    pub modified: u64,
    pub created: u64,
    pub app: Option<String>,
    pub transcription_status: String,
    pub completed_chunks: u32,
    pub chunk_count: u32,
    pub transcript_text: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ExcludedApp {
    pub name: String,
    pub excluded: bool,
}

fn timestamp_millis(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

pub fn database_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join("Mic Monitor").join("index.sqlite3"))
}

fn connection() -> rusqlite::Result<Connection> {
    let path = database_path()
        .ok_or_else(|| rusqlite::Error::InvalidPath(PathBuf::from("data directory")))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    }
    let connection = Connection::open(path)?;
    connection.busy_timeout(std::time::Duration::from_secs(2))?;
    connection.execute_batch(
        "PRAGMA journal_mode = WAL;
         CREATE TABLE IF NOT EXISTS recordings (
             id INTEGER PRIMARY KEY,
             name TEXT NOT NULL UNIQUE,
             path TEXT NOT NULL,
             size INTEGER NOT NULL,
             modified INTEGER NOT NULL,
             created INTEGER NOT NULL,
             app TEXT,
             transcription_status TEXT NOT NULL DEFAULT 'none',
             completed_chunks INTEGER NOT NULL DEFAULT 0,
             chunk_count INTEGER NOT NULL DEFAULT 0,
             transcript_text TEXT
         );
         CREATE TABLE IF NOT EXISTS excluded_apps (
             name TEXT PRIMARY KEY,
             excluded INTEGER NOT NULL DEFAULT 1
         );
         CREATE TABLE IF NOT EXISTS settings (
             key TEXT PRIMARY KEY,
             value TEXT NOT NULL
         );",
    )?;
    let _ = connection.execute(
        "ALTER TABLE recordings ADD COLUMN transcription_status TEXT NOT NULL DEFAULT 'none'",
        [],
    );
    let _ = connection.execute(
        "ALTER TABLE recordings ADD COLUMN completed_chunks INTEGER NOT NULL DEFAULT 0",
        [],
    );
    let _ = connection.execute(
        "ALTER TABLE recordings ADD COLUMN chunk_count INTEGER NOT NULL DEFAULT 0",
        [],
    );
    let _ = connection.execute("ALTER TABLE recordings ADD COLUMN transcript_text TEXT", []);
    Ok(connection)
}

fn app_from_name(name: &str) -> Option<String> {
    let stem = name.strip_suffix(".m4a")?;
    stem.get(17..)
        .map(str::trim)
        .filter(|app| !app.is_empty())
        .map(str::to_owned)
}

pub fn reconcile(folder: &Path) -> rusqlite::Result<Vec<RecordingMetadata>> {
    let connection = connection()?;
    let transaction = connection.unchecked_transaction()?;
    let mut seen = Vec::new();
    if let Ok(entries) = std::fs::read_dir(folder) {
        for entry in entries.flatten() {
            let path = entry.path();
            let metadata = match entry.metadata() {
                Ok(metadata)
                    if metadata.is_file()
                        && path.extension().and_then(|e| e.to_str()) == Some("m4a") =>
                {
                    metadata
                }
                _ => continue,
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            let modified = metadata
                .modified()
                .map(timestamp_millis)
                .unwrap_or_default();
            let created = metadata.created().map(timestamp_millis).unwrap_or(modified);
            transaction.execute(
                "INSERT INTO recordings (name,path,size,modified,created,app) VALUES (?1,?2,?3,?4,?5,?6)
                 ON CONFLICT(name) DO UPDATE SET path=excluded.path,size=excluded.size,modified=excluded.modified,created=excluded.created,app=excluded.app",
                params![name, path.to_string_lossy(), metadata.len() as i64, modified, created, app_from_name(&name)],
            )?;
            seen.push(name);
        }
    }
    if seen.is_empty() {
        transaction.execute("DELETE FROM recordings", [])?;
    } else {
        let placeholders = std::iter::repeat("?")
            .take(seen.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!("DELETE FROM recordings WHERE name NOT IN ({placeholders})");
        let refs: Vec<&dyn rusqlite::ToSql> = seen
            .iter()
            .map(|name| name as &dyn rusqlite::ToSql)
            .collect();
        transaction.execute(&sql, refs.as_slice())?;
    }
    transaction.commit()?;
    list()
}

pub fn list() -> rusqlite::Result<Vec<RecordingMetadata>> {
    let connection = connection()?;
    let mut statement = connection.prepare("SELECT id,name,path,size,modified,created,app,transcription_status,completed_chunks,chunk_count,transcript_text FROM recordings ORDER BY modified DESC, name DESC")?;
    let rows = statement
        .query_map([], |row| {
            Ok(RecordingMetadata {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                size: row.get::<_, i64>(3)? as u64,
                modified: row.get::<_, i64>(4)? as u64,
                created: row.get::<_, i64>(5)? as u64,
                app: row.get(6)?,
                transcription_status: row.get(7)?,
                completed_chunks: row.get::<_, i64>(8)? as u32,
                chunk_count: row.get::<_, i64>(9)? as u32,
                transcript_text: row.get(10)?,
            })
        })?
        .collect();
    rows
}

pub fn set_transcription(
    name: &str,
    status: &str,
    completed: u32,
    total: u32,
    text: Option<&str>,
) -> rusqlite::Result<()> {
    let connection = connection()?;
    connection.execute(
        "UPDATE recordings SET transcription_status=?1,completed_chunks=?2,chunk_count=?3,transcript_text=?4 WHERE name=?5",
        params![status, completed, total, text, name],
    )?;
    Ok(())
}

pub fn rename(old: &str, new: &str, path: &Path) -> rusqlite::Result<()> {
    let connection = connection()?;
    connection.execute(
        "UPDATE recordings SET name=?1,path=?2 WHERE name=?3",
        params![new, path.to_string_lossy(), old],
    )?;
    Ok(())
}

pub fn remove(name: &str) -> rusqlite::Result<()> {
    let connection = connection()?;
    connection.execute("DELETE FROM recordings WHERE name=?1", [name])?;
    Ok(())
}

pub fn excluded() -> rusqlite::Result<Vec<ExcludedApp>> {
    let connection = connection()?;
    let mut statement = connection.prepare("SELECT name,excluded FROM excluded_apps ORDER BY name")?;
    let rows = statement
        .query_map([], |row| {
            Ok(ExcludedApp {
                name: row.get(0)?,
                excluded: row.get::<_, i64>(1)? != 0,
            })
        })?
        .collect();
    rows
}

pub fn set_excluded(name: &str, value: bool) -> rusqlite::Result<()> {
    let connection = connection()?;
    connection.execute("INSERT INTO excluded_apps(name,excluded) VALUES (?1,?2) ON CONFLICT(name) DO UPDATE SET excluded=?2", params![name, value as i64])?;
    Ok(())
}

pub fn is_excluded(name: &str) -> bool {
    connection()
        .ok()
        .and_then(|c| {
            c.query_row(
                "SELECT excluded FROM excluded_apps WHERE name=?1",
                [name],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .ok()
            .flatten()
        })
        .is_some_and(|excluded| excluded != 0)
}

pub fn setting(key: &str) -> rusqlite::Result<Option<String>> {
    let connection = connection()?;
    connection
        .query_row("SELECT value FROM settings WHERE key=?1", [key], |row| {
            row.get(0)
        })
        .optional()
}

pub fn set_setting(key: &str, value: &str) -> rusqlite::Result<()> {
    let connection = connection()?;
    connection.execute("INSERT INTO settings(key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key, value])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::app_from_name;
    #[test]
    fn extracts_app_from_recording_name() {
        assert_eq!(
            app_from_name("2026-11-08 09-05 Zoom.m4a").as_deref(),
            Some("Zoom")
        );
    }
    #[test]
    fn ignores_non_recording_names() {
        assert_eq!(app_from_name("recording.m4a"), None);
    }
}
