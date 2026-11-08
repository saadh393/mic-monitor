#[derive(Debug, PartialEq)]
pub enum Reason {
    Empty,
    InvalidCharacter,
    LeadingDot,
    Exists,
}

impl Reason {
    pub fn code(&self) -> &'static str {
        match self {
            Reason::Empty => "empty",
            Reason::InvalidCharacter => "invalid_character",
            Reason::LeadingDot => "leading_dot",
            Reason::Exists => "exists",
        }
    }
}

pub fn target(dir_names: &[&str], old: &str, new: &str) -> Result<Option<String>, Reason> {
    let name = new.trim();
    if name.is_empty() {
        return Err(Reason::Empty);
    }
    if name.contains(['/', ':']) {
        return Err(Reason::InvalidCharacter);
    }
    if name.starts_with('.') {
        return Err(Reason::LeadingDot);
    }
    let file = format!("{name}.m4a");
    if file == old {
        return Ok(None);
    }
    let lower = file.to_lowercase();
    if dir_names.iter().any(|existing| *existing != old && existing.to_lowercase() == lower) {
        return Err(Reason::Exists);
    }
    Ok(Some(file))
}

#[cfg(test)]
mod tests {
    use super::{target, Reason};

    fn renamed(name: &str) -> Result<Option<String>, Reason> {
        Ok(Some(name.to_string()))
    }

    #[test]
    fn keeps_extension() {
        assert_eq!(target(&["a.m4a"], "a.m4a", "Standup"), renamed("Standup.m4a"));
    }

    #[test]
    fn trims_whitespace() {
        assert_eq!(target(&["a.m4a"], "a.m4a", "  Standup  "), renamed("Standup.m4a"));
    }

    #[test]
    fn empty_rejected() {
        assert_eq!(target(&["a.m4a"], "a.m4a", ""), Err(Reason::Empty));
        assert_eq!(target(&["a.m4a"], "a.m4a", "   "), Err(Reason::Empty));
    }

    #[test]
    fn slash_rejected() {
        assert_eq!(target(&["a.m4a"], "a.m4a", "a/b"), Err(Reason::InvalidCharacter));
    }

    #[test]
    fn colon_rejected() {
        assert_eq!(target(&["a.m4a"], "a.m4a", "a:b"), Err(Reason::InvalidCharacter));
    }

    #[test]
    fn leading_dot_rejected() {
        assert_eq!(target(&["a.m4a"], "a.m4a", ".hidden"), Err(Reason::LeadingDot));
        assert_eq!(target(&["a.m4a"], "a.m4a", ".."), Err(Reason::LeadingDot));
    }

    #[test]
    fn existing_name_rejected() {
        assert_eq!(target(&["a.m4a", "b.m4a"], "a.m4a", "b"), Err(Reason::Exists));
    }

    #[test]
    fn existing_name_any_case_rejected() {
        assert_eq!(target(&["a.m4a", "B.m4a"], "a.m4a", "b"), Err(Reason::Exists));
    }

    #[test]
    fn existing_non_m4a_ignored() {
        assert_eq!(target(&["a.m4a", "b.txt"], "a.m4a", "b"), renamed("b.m4a"));
    }

    #[test]
    fn same_name_is_noop() {
        assert_eq!(target(&["a.m4a"], "a.m4a", "a"), Ok(None));
        assert_eq!(target(&["a.m4a"], "a.m4a", " a "), Ok(None));
    }

    #[test]
    fn case_only_change_allowed() {
        assert_eq!(target(&["zoom.m4a"], "zoom.m4a", "Zoom"), renamed("Zoom.m4a"));
    }

    #[test]
    fn typed_extension_kept_as_text() {
        assert_eq!(target(&["a.m4a"], "a.m4a", "x.m4a"), renamed("x.m4a.m4a"));
    }
}
