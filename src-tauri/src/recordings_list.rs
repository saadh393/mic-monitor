use std::time::SystemTime;

fn stem(name: &str) -> &str {
    name.strip_suffix(".m4a").unwrap_or(name)
}

pub fn list<T>(entries: Vec<(String, SystemTime, T)>, recording: Option<&str>) -> Vec<(String, SystemTime, T)> {
    let mut kept: Vec<_> = entries.into_iter().filter(|(name, _, _)| name.ends_with(".m4a") && Some(name.as_str()) != recording).collect();
    kept.sort_by(|a, b| (b.1, stem(&b.0)).cmp(&(a.1, stem(&a.0))));
    kept
}

#[cfg(test)]
mod tests {
    use super::list;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn entries(items: &[(&str, u64)]) -> Vec<(String, SystemTime, ())> {
        items.iter().map(|(name, seconds)| (name.to_string(), at(*seconds), ())).collect()
    }

    fn names(listed: Vec<(String, SystemTime, ())>) -> Vec<String> {
        listed.into_iter().map(|(name, _, _)| name).collect()
    }

    #[test]
    fn keeps_only_m4a() {
        let input = entries(&[("a.m4a", 1), ("b.txt", 2), (".DS_Store", 3), ("c.M4A.tmp", 4)]);
        assert_eq!(names(list(input, None)), vec!["a.m4a"]);
    }

    #[test]
    fn newest_first() {
        let input = entries(&[("old.m4a", 1), ("new.m4a", 3), ("mid.m4a", 2)]);
        assert_eq!(names(list(input, None)), vec!["new.m4a", "mid.m4a", "old.m4a"]);
    }

    #[test]
    fn same_time_sorted_by_name_desc() {
        let input = entries(&[("2026-11-08 09-05 Zoom.m4a", 5), ("2026-11-08 09-05 Zoom 2.m4a", 5)]);
        assert_eq!(names(list(input, None)), vec!["2026-11-08 09-05 Zoom 2.m4a", "2026-11-08 09-05 Zoom.m4a"]);
    }

    #[test]
    fn excludes_file_being_recorded() {
        let input = entries(&[("a.m4a", 1), ("x.m4a", 2), ("b.m4a", 3)]);
        assert_eq!(names(list(input, Some("x.m4a"))), vec!["b.m4a", "a.m4a"]);
    }

    #[test]
    fn empty_input() {
        assert!(list(entries(&[]), None).is_empty());
    }

    #[test]
    fn many_files() {
        let input: Vec<_> = (0..500).map(|n| (format!("{n:03}.m4a"), at(n % 7), ())).collect();
        let listed = list(input, None);
        assert_eq!(listed.len(), 500);
        assert!(listed.windows(2).all(|pair| (pair[0].1, &pair[0].0) > (pair[1].1, &pair[1].0)));
        assert_eq!(listed[0].0, "496.m4a");
    }
}
