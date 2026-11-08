pub fn from_label(label: &str) -> Option<String> {
    let (title, status) = label.rsplit_once(" - ")?;
    status.to_lowercase().contains("microphone").then(|| title.to_string())
}

#[cfg(test)]
mod tests {
    use super::from_label;

    #[test]
    fn extracts_title_of_recording_tab() {
        assert_eq!(from_label("Meet - abc-defg-hij - Microphone recording").as_deref(), Some("Meet - abc-defg-hij"));
        assert_eq!(from_label("Zoom - Camera or microphone recording").as_deref(), Some("Zoom"));
    }

    #[test]
    fn ignores_other_tabs() {
        assert_eq!(from_label("YouTube - Camera recording"), None);
        assert_eq!(from_label("New Tab"), None);
    }
}
