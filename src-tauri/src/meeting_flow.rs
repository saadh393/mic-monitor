use std::time::{Duration, Instant};

use crate::mic_session::MicApp;

pub const PROMPT_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Default, PartialEq, Debug)]
pub struct Flow {
    asking: Option<(MicApp, Instant)>,
    accepted: Option<i32>,
}

pub enum Input {
    SessionStart(MicApp),
    SessionEnd(i32),
    Yes,
    No,
    Tick,
    Stop,
}

#[derive(PartialEq, Debug)]
pub enum Action {
    ShowPrompt(String),
    HidePrompt,
    Accept(MicApp),
    EndRecording,
}

fn hidden(accepted: Option<i32>) -> (Flow, Vec<Action>) {
    (Flow { asking: None, accepted }, vec![Action::HidePrompt])
}

pub fn step(flow: Flow, input: Input, now: Instant) -> (Flow, Vec<Action>) {
    match (input, flow.asking) {
        (Input::SessionStart(app), None) if flow.accepted.is_none() => {
            let text = format!("Record {}?", app.labels.join(", "));
            (Flow { asking: Some((app, now + PROMPT_TIMEOUT)), accepted: None }, vec![Action::ShowPrompt(text)])
        }
        (Input::SessionEnd(pid), Some((app, _))) if app.pid == pid => hidden(None),
        (Input::SessionEnd(pid), None) if flow.accepted == Some(pid) => (Flow::default(), vec![Action::EndRecording]),
        (Input::Stop, None) if flow.accepted.is_some() => (Flow::default(), vec![Action::EndRecording]),
        (Input::Yes, Some((app, _))) => {
            let (flow, mut actions) = hidden(Some(app.pid));
            actions.push(Action::Accept(app));
            (flow, actions)
        }
        (Input::No, Some(_)) => hidden(None),
        (Input::Tick, Some((_, until))) if now >= until => hidden(None),
        (_, asking) => (Flow { asking, accepted: flow.accepted }, vec![]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(pid: i32, labels: &[&str]) -> MicApp {
        MicApp { pid, name: labels[0].into(), labels: labels.iter().map(|l| l.to_string()).collect() }
    }

    fn a() -> MicApp {
        app(1, &["A"])
    }

    fn b() -> MicApp {
        app(2, &["B"])
    }

    fn run(inputs: Vec<(u64, Input)>) -> (Flow, Vec<Vec<Action>>) {
        let t = Instant::now();
        let mut flow = Flow::default();
        let mut out = Vec::new();
        for (secs, input) in inputs {
            let (next, actions) = step(flow, input, t + Duration::from_secs(secs));
            flow = next;
            out.push(actions);
        }
        (flow, out)
    }

    fn show(text: &str) -> Vec<Action> {
        vec![Action::ShowPrompt(text.into())]
    }

    #[test]
    fn prompts_on_session_start() {
        let (_, out) = run(vec![(0, Input::SessionStart(a()))]);
        assert_eq!(out, vec![show("Record A?")]);
    }

    #[test]
    fn no_prompt_while_asking() {
        let (flow, out) = run(vec![(0, Input::SessionStart(a())), (1, Input::SessionStart(b())), (2, Input::No), (3, Input::Tick), (20, Input::Tick)]);
        assert!(out[1].is_empty());
        assert_eq!(out[2], vec![Action::HidePrompt]);
        assert!(out[3..].iter().all(Vec::is_empty));
        assert_eq!(flow, Flow::default());
    }

    #[test]
    fn no_prompt_while_accepted() {
        let (_, out) = run(vec![(0, Input::SessionStart(a())), (1, Input::Yes), (2, Input::SessionStart(b()))]);
        assert!(out[2].is_empty());
    }

    #[test]
    fn no_hides() {
        let (_, out) = run(vec![(0, Input::SessionStart(a())), (1, Input::No), (2, Input::SessionStart(b()))]);
        assert_eq!(out[1], vec![Action::HidePrompt]);
        assert_eq!(out[2], show("Record B?"));
    }

    #[test]
    fn timeout_hides_after_15s() {
        let (flow, out) = run(vec![(0, Input::SessionStart(a())), (14, Input::Tick), (15, Input::Tick)]);
        assert!(out[1].is_empty());
        assert_eq!(out[2], vec![Action::HidePrompt]);
        assert_eq!(flow, Flow::default());
    }

    #[test]
    fn yes_accepts() {
        let (_, out) = run(vec![(0, Input::SessionStart(a())), (1, Input::Yes)]);
        assert_eq!(out[1], vec![Action::HidePrompt, Action::Accept(a())]);
    }

    #[test]
    fn yes_or_no_when_not_asking_does_nothing() {
        let (flow, out) = run(vec![(0, Input::Yes), (1, Input::No)]);
        assert!(out.iter().all(Vec::is_empty));
        assert_eq!(flow, Flow::default());
        let (_, out) = run(vec![(0, Input::SessionStart(a())), (15, Input::Tick), (16, Input::Yes), (17, Input::No)]);
        assert!(out[2..].iter().all(Vec::is_empty));
    }

    #[test]
    fn session_end_hides_its_prompt() {
        let (flow, out) = run(vec![(0, Input::SessionStart(a())), (1, Input::SessionEnd(1))]);
        assert_eq!(out[1], vec![Action::HidePrompt]);
        assert_eq!(flow, Flow::default());
    }

    #[test]
    fn other_session_end_keeps_prompt() {
        let (flow, out) = run(vec![(0, Input::SessionStart(a())), (1, Input::SessionEnd(2)), (2, Input::Yes)]);
        assert!(out[1].is_empty());
        assert_eq!(out[2], vec![Action::HidePrompt, Action::Accept(a())]);
        assert_ne!(flow, Flow::default());
    }

    #[test]
    fn accepted_session_end_clears_accepted() {
        let (_, out) = run(vec![(0, Input::SessionStart(a())), (1, Input::Yes), (2, Input::SessionEnd(1)), (3, Input::SessionStart(b()))]);
        assert_eq!(out[2], vec![Action::EndRecording]);
        assert_eq!(out[3], show("Record B?"));
    }

    #[test]
    fn prompt_text_lists_tabs() {
        let (_, out) = run(vec![(0, Input::SessionStart(app(1, &["Google Chrome — X", "Google Chrome — Y"])))]);
        assert_eq!(out, vec![show("Record Google Chrome — X, Google Chrome — Y?")]);
    }

    fn accepted() -> Vec<(u64, Input)> {
        vec![(0, Input::SessionStart(a())), (1, Input::Yes)]
    }

    fn after_accepted(more: Vec<(u64, Input)>) -> (Flow, Vec<Vec<Action>>) {
        let mut inputs = accepted();
        inputs.extend(more);
        let (flow, mut out) = run(inputs);
        (flow, out.split_off(2))
    }

    #[test]
    fn stop_ends_recording_once() {
        let (flow, out) = after_accepted(vec![(2, Input::Stop), (3, Input::Stop)]);
        assert_eq!(out, vec![vec![Action::EndRecording], vec![]]);
        assert_eq!(flow, Flow::default());
    }

    #[test]
    fn accepted_session_end_ends_recording() {
        let (flow, out) = after_accepted(vec![(2, Input::SessionEnd(1))]);
        assert_eq!(out, vec![vec![Action::EndRecording]]);
        assert_eq!(flow, Flow::default());
    }

    #[test]
    fn stop_then_session_end_ends_once() {
        let (_, out) = after_accepted(vec![(2, Input::Stop), (3, Input::SessionEnd(1))]);
        assert_eq!(out, vec![vec![Action::EndRecording], vec![]]);
    }

    #[test]
    fn session_end_then_stop_ends_once() {
        let (flow, out) = after_accepted(vec![(2, Input::SessionEnd(1)), (3, Input::Stop)]);
        assert_eq!(out, vec![vec![Action::EndRecording], vec![]]);
        assert_eq!(flow, Flow::default());
    }

    #[test]
    fn other_session_end_does_not_end_recording() {
        let (_, out) = after_accepted(vec![(2, Input::SessionEnd(2)), (3, Input::Stop)]);
        assert_eq!(out, vec![vec![], vec![Action::EndRecording]]);
    }

    #[test]
    fn stop_when_idle_does_nothing() {
        let (flow, out) = run(vec![(0, Input::Stop)]);
        assert_eq!(out, vec![vec![]]);
        assert_eq!(flow, Flow::default());
        let (_, out) = run(vec![(0, Input::SessionStart(a())), (1, Input::Stop), (2, Input::Yes)]);
        assert!(out[1].is_empty());
        assert_eq!(out[2], vec![Action::HidePrompt, Action::Accept(a())]);
    }

    #[test]
    fn session_started_while_recording_not_prompted_after_stop() {
        let (_, out) = after_accepted(vec![(2, Input::SessionStart(b())), (3, Input::Stop), (4, Input::Tick), (20, Input::Tick)]);
        assert_eq!(out, vec![vec![], vec![Action::EndRecording], vec![], vec![]]);
    }
}
