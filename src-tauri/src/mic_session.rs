use std::collections::BTreeMap;
use std::time::{Duration, Instant};

pub const GAP: Duration = Duration::from_secs(5);

#[derive(Clone, PartialEq, Debug, serde::Serialize)]
pub struct MicApp {
    pub pid: i32,
    pub name: String,
    pub labels: Vec<String>,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Event {
    Start(MicApp),
    End(MicApp),
}

#[derive(Default)]
pub struct Sessions(BTreeMap<i32, (MicApp, Option<Instant>)>);

pub fn step(sessions: Sessions, present: &[MicApp], now: Instant) -> (Sessions, Vec<Event>) {
    let mut kept = BTreeMap::new();
    let mut ends = Vec::new();
    for (pid, (app, absent_since)) in sessions.0 {
        if present.iter().any(|p| p.pid == pid) {
            kept.insert(pid, (app, None));
            continue;
        }
        let since = absent_since.unwrap_or(now);
        if now.duration_since(since) >= GAP {
            ends.push(Event::End(app));
        } else {
            kept.insert(pid, (app, Some(since)));
        }
    }
    let mut starts = Vec::new();
    for app in present {
        if !kept.contains_key(&app.pid) {
            starts.push(Event::Start(app.clone()));
        }
        kept.insert(app.pid, (app.clone(), None));
    }
    ends.extend(starts);
    (Sessions(kept), ends)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(pid: i32, name: &str, labels: &[&str]) -> MicApp {
        MicApp { pid, name: name.into(), labels: labels.iter().map(|l| l.to_string()).collect() }
    }

    fn a() -> MicApp {
        app(1, "A", &["A"])
    }

    fn b() -> MicApp {
        app(2, "B", &["B"])
    }

    fn run_at(ticks: &[(u64, Vec<MicApp>)]) -> Vec<Vec<Event>> {
        let t = Instant::now();
        let mut sessions = Sessions::default();
        let mut out = Vec::new();
        for (ms, present) in ticks {
            let (next, events) = step(sessions, present, t + Duration::from_millis(*ms));
            sessions = next;
            out.push(events);
        }
        out
    }

    fn run(ticks: &[Vec<MicApp>]) -> Vec<Vec<Event>> {
        let timed: Vec<_> = ticks.iter().enumerate().map(|(n, p)| (1000 * n as u64, p.clone())).collect();
        run_at(&timed)
    }

    fn quiet(events: &[Vec<Event>]) -> bool {
        events.iter().all(Vec::is_empty)
    }

    #[test]
    fn starts_when_app_appears() {
        assert_eq!(run(&[vec![a()]]), vec![vec![Event::Start(a())]]);
    }

    #[test]
    fn no_repeat_start_while_present() {
        let events = run(&vec![vec![a()]; 11]);
        assert_eq!(events[0], vec![Event::Start(a())]);
        assert!(quiet(&events[1..]));
    }

    #[test]
    fn gap_of_5s_keeps_session() {
        let events = run(&[vec![a()], vec![], vec![], vec![], vec![], vec![a()], vec![a()]]);
        assert!(quiet(&events[1..]));
        let events = run(&[vec![a()], vec![], vec![a()], vec![a()]]);
        assert!(quiet(&events[1..]));
    }

    #[test]
    fn gap_over_5s_ends_session() {
        let mut ticks = vec![vec![a()]];
        ticks.extend(vec![vec![]; 10]);
        let events = run(&ticks);
        assert!(quiet(&events[1..6]));
        assert_eq!(events[6], vec![Event::End(a())]);
        assert!(quiet(&events[7..]));
    }

    #[test]
    fn restart_after_end_is_new_session() {
        let mut ticks = vec![vec![a()]];
        ticks.extend(vec![vec![]; 6]);
        ticks.push(vec![a()]);
        let events = run(&ticks);
        assert_eq!(events[6], vec![Event::End(a())]);
        assert_eq!(events[7], vec![Event::Start(a())]);
    }

    #[test]
    fn apps_are_independent() {
        let mut ticks = vec![vec![a(), b()]];
        ticks.extend(vec![vec![a()]; 8]);
        let events = run(&ticks);
        assert_eq!(events[0], vec![Event::Start(a()), Event::Start(b())]);
        assert!(quiet(&events[1..6]));
        assert_eq!(events[6], vec![Event::End(b())]);
        assert!(quiet(&events[7..]));
    }

    #[test]
    fn label_change_is_not_a_new_session() {
        let changed = app(1, "A", &["A — x", "A — y"]);
        let mut ticks = vec![vec![a()], vec![changed.clone()]];
        ticks.extend(vec![vec![]; 6]);
        let events = run(&ticks);
        assert!(events[1].is_empty());
        assert_eq!(events[7], vec![Event::End(changed)]);
    }

    #[test]
    fn same_tick_end_and_start() {
        let events = run(&[vec![a()], vec![], vec![], vec![], vec![], vec![], vec![b()]]);
        assert_eq!(events[6], vec![Event::End(a()), Event::Start(b())]);
    }

    #[test]
    fn new_pid_is_new_session() {
        let other = app(2, "A", &["A"]);
        let mut ticks = vec![vec![a()]];
        ticks.extend(vec![vec![other.clone()]; 7]);
        let events = run(&ticks);
        assert_eq!(events[1], vec![Event::Start(other)]);
        assert!(quiet(&events[2..6]));
        assert_eq!(events[6], vec![Event::End(a())]);
        assert!(quiet(&events[7..]));
    }

    #[test]
    fn stall_does_not_split_present_app() {
        let events = run_at(&[(0, vec![a()]), (20_000, vec![a()])]);
        assert!(events[1].is_empty());
    }

    #[test]
    fn end_counts_from_first_absence() {
        let mut ticks = vec![(0, vec![a()])];
        ticks.extend((1..=10).map(|k| (1010 * k, vec![])));
        let events = run_at(&ticks);
        assert!(quiet(&events[1..6]));
        assert_eq!(events[6], vec![Event::End(a())]);
        assert!(quiet(&events[7..]));
    }
}
