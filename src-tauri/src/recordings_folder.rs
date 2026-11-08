use std::fs::{self, DirEntry};
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub fn path() -> Option<PathBuf> {
    dirs::document_dir().map(|documents| documents.join("Mic Monitor"))
}

fn file(entry: io::Result<DirEntry>) -> Option<(String, SystemTime, u64)> {
    let entry = entry.ok()?;
    let metadata = entry.metadata().ok().filter(|metadata| metadata.is_file())?;
    Some((entry.file_name().to_string_lossy().into_owned(), metadata.modified().ok()?, metadata.len()))
}

pub fn read(folder: &Path) -> io::Result<Vec<(String, SystemTime, u64)>> {
    match fs::read_dir(folder) {
        Ok(entries) => Ok(entries.filter_map(file).collect()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}
