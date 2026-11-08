pub fn of(users: &[String], recording: bool) -> Option<String> {
    let mut parts = Vec::new();
    if recording {
        parts.push("🔴".to_string());
    }
    if !users.is_empty() {
        parts.push(format!("🎙 {}", users.join(", ")));
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

#[cfg(test)]
mod tests {
    use super::of;

    fn quicktime() -> Vec<String> {
        vec!["QuickTime Player".into()]
    }

    #[test]
    fn empty_when_idle() {
        assert_eq!(of(&[], false), None);
    }

    #[test]
    fn lists_users() {
        assert_eq!(of(&quicktime(), false).as_deref(), Some("🎙 QuickTime Player"));
    }

    #[test]
    fn marks_recording() {
        assert_eq!(of(&quicktime(), true).as_deref(), Some("🔴 🎙 QuickTime Player"));
        assert_eq!(of(&[], true).as_deref(), Some("🔴"));
    }
}
