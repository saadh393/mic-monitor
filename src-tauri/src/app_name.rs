pub fn from_path(path: &str) -> String {
    path.split('/')
        .find_map(|part| part.strip_suffix(".app"))
        .or_else(|| path.rsplit('/').next())
        .unwrap_or(path)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::from_path;

    #[test]
    fn uses_outermost_app_bundle() {
        assert_eq!(
            from_path("/Applications/Google Chrome.app/Contents/Frameworks/Google Chrome Helper.app/Contents/MacOS/Google Chrome Helper"),
            "Google Chrome"
        );
    }

    #[test]
    fn falls_back_to_binary_name() {
        assert_eq!(from_path("/usr/local/bin/ffmpeg"), "ffmpeg");
    }
}
