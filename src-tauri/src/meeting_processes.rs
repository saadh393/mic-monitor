pub fn select(processes: &[(u32, String)], app: &str) -> Vec<u32> {
    processes.iter().filter(|(_, name)| name == app).map(|&(id, _)| id).collect()
}

#[cfg(test)]
mod tests {
    use super::select;

    fn list(items: &[(u32, &str)]) -> Vec<(u32, String)> {
        items.iter().map(|&(id, name)| (id, name.to_string())).collect()
    }

    #[test]
    fn includes_helpers_of_same_app() {
        assert_eq!(select(&list(&[(10, "Google Chrome"), (11, "Google Chrome"), (12, "Music")]), "Google Chrome"), vec![10, 11]);
    }

    #[test]
    fn excludes_other_apps() {
        assert_eq!(select(&list(&[(12, "Music"), (13, "Safari")]), "Google Chrome"), Vec::<u32>::new());
    }

    #[test]
    fn matches_whole_name() {
        assert_eq!(select(&list(&[(14, "Google Chrome Canary")]), "Google Chrome"), Vec::<u32>::new());
    }
}
