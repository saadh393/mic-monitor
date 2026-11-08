use chrono::NaiveDateTime;

pub fn file_name(start: NaiveDateTime, app: &str, existing: &[String]) -> String {
    let base = format!("{} {}", start.format("%Y-%m-%d %H-%M"), app.replace(['/', ':'], "-"));
    let mut name = format!("{base}.m4a");
    let mut n = 2;
    while existing.contains(&name) {
        name = format!("{base} {n}.m4a");
        n += 1;
    }
    name
}

#[cfg(test)]
mod tests {
    use super::file_name;
    use chrono::NaiveDate;

    fn start() -> chrono::NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, 8).unwrap().and_hms_opt(9, 5, 0).unwrap()
    }

    #[test]
    fn formats_date_time_and_app() {
        assert_eq!(file_name(start(), "QuickTime Player", &[]), "2026-11-08 09-05 QuickTime Player.m4a");
    }

    #[test]
    fn replaces_path_characters() {
        assert_eq!(file_name(start(), "A/B:C", &[]), "2026-11-08 09-05 A-B-C.m4a");
    }

    #[test]
    fn does_not_overwrite() {
        let taken = "2026-11-08 09-05 QuickTime Player.m4a".to_string();
        assert_eq!(file_name(start(), "QuickTime Player", &[taken.clone()]), "2026-11-08 09-05 QuickTime Player 2.m4a");
        assert_eq!(file_name(start(), "QuickTime Player", &[taken, "2026-11-08 09-05 QuickTime Player 2.m4a".into()]), "2026-11-08 09-05 QuickTime Player 3.m4a");
    }
}
