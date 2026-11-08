use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};

use crate::meeting_flow::{self, Action, Flow, Input};
use crate::recording::{self, Recording};
use crate::{monitor, panel};

fn could_not_save(app: &AppHandle, error: String) {
    eprintln!("{error}");
    panel::show(app, panel::Payload { kind: "save-failed", text: "Recording could not be saved".into(), path: None });
}

fn finish(app: &AppHandle) -> Option<Result<(PathBuf, u64), String>> {
    let recording = app.state::<Mutex<Option<Recording>>>().lock().unwrap().take()?;
    let stopped = recording.stop();
    if let Ok((path, 0)) = &stopped {
        let _ = std::fs::remove_file(path);
    }
    Some(stopped)
}

fn end_recording(app: &AppHandle) {
    match finish(app) {
        None | Some(Ok((_, 0))) => {}
        Some(Ok((path, _))) if path.is_file() => {
            let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            panel::show(app, panel::Payload { kind: "saved", text: format!("Saved — {name}"), path: Some(path.to_string_lossy().into_owned()) });
        }
        Some(Ok((path, _))) => could_not_save(app, format!("the recording {} is no longer in its folder", path.display())),
        Some(Err(e)) => could_not_save(app, format!("could not save the recording: {e}")),
    }
}

pub fn quit(app: &AppHandle) {
    let state = app.state::<Mutex<Flow>>();
    let mut flow = state.lock().unwrap();
    let (next, actions) = meeting_flow::step(std::mem::take(&mut *flow), Input::Stop, Instant::now());
    *flow = next;
    if actions.contains(&Action::EndRecording) {
        if let Some(Err(e)) = finish(app) {
            eprintln!("could not save the recording: {e}");
        }
    }
}

pub fn handle(app: &AppHandle, input: Input) {
    let state = app.state::<Mutex<Flow>>();
    let mut flow = state.lock().unwrap();
    let (next, actions) = meeting_flow::step(std::mem::take(&mut *flow), input, Instant::now());
    *flow = next;
    for action in actions {
        match action {
            Action::ShowPrompt(text) => panel::show(app, panel::Payload { kind: "prompt", text, path: None }),
            Action::HidePrompt => panel::hide(app),
            Action::Accept(session) => {
                recording::start(app, &session);
                let _ = app.emit("recording-changed", ());
                monitor::wake(app);
            }
            Action::EndRecording => {
                end_recording(app);
                let _ = app.emit("recording-changed", ());
                monitor::wake(app);
            }
        }
    }
}
