use std::path::Path;

pub fn inside(folder: &Path, path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".m4a") && folder.join(name).as_os_str() == path.as_os_str())
}

#[cfg(test)]
mod tests {
    use super::inside;
    use std::path::Path;

    const FOLDER: &str = "/Users/me/Documents/Mic Monitor";

    fn allowed(path: &str) -> bool {
        inside(Path::new(FOLDER), Path::new(path))
    }

    #[test]
    fn file_in_folder_allowed() {
        assert!(allowed(&format!("{FOLDER}/a.m4a")));
    }

    #[test]
    fn other_extension_refused() {
        assert!(!allowed(&format!("{FOLDER}/a.txt")));
        assert!(!allowed(&format!("{FOLDER}/a.M4A.tmp")));
        assert!(!allowed(&format!("{FOLDER}/a")));
    }

    #[test]
    fn subfolder_refused() {
        assert!(!allowed(&format!("{FOLDER}/sub/a.m4a")));
    }

    #[test]
    fn traversal_refused() {
        assert!(!allowed(&format!("{FOLDER}/../a.m4a")));
        assert!(!allowed(&format!("{FOLDER}/sub/../a.m4a")));
        assert!(!allowed(&format!("{FOLDER}/./a.m4a")));
    }

    #[test]
    fn other_folder_refused() {
        assert!(!allowed("/tmp/a.m4a"));
        assert!(!allowed("~/Documents/a.m4a"));
        assert!(!allowed("/Users/me/Documents/a.m4a"));
        assert!(!allowed(&format!("{FOLDER}.m4a")));
    }

    #[test]
    fn relative_refused() {
        assert!(!allowed("a.m4a"));
    }

    #[test]
    fn folder_itself_refused() {
        assert!(!allowed(FOLDER));
        assert!(!allowed(&format!("{FOLDER}/")));
    }
}
