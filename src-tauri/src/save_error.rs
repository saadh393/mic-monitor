pub fn is_privacy(os_error: Option<i32>) -> bool {
    os_error == Some(libc::EPERM)
}

#[cfg(test)]
mod tests {
    use super::is_privacy;

    #[test]
    fn operation_not_permitted_is_privacy() {
        assert!(is_privacy(Some(1)));
    }

    #[test]
    fn other_errors_are_not_privacy() {
        assert!(!is_privacy(Some(13)));
        assert!(!is_privacy(Some(28)));
        assert!(!is_privacy(None));
    }
}
