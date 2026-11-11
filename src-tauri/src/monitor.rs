use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

use crate::meeting_flow::Input;
use crate::mic_session::{self, Event, MicApp, Sessions};
use crate::{
    accessibility, app_name, meeting, metadata_index, mic, own_process, parent_pid, process_path,
    recording, tray_menu, tray_title,
};

pub fn name_of(pid: i32) -> Option<String> {
    process_path::of(pid).map(|path| app_name::from_path(&path))
}

fn app_process(pid: i32) -> Option<(i32, String)> {
    let name = name_of(pid)?;
    let parent =
        parent_pid::of(pid).filter(|&parent| name_of(parent).as_deref() == Some(name.as_str()));
    Some((parent.unwrap_or(pid), name))
}

fn labels(pid: i32, name: String) -> Vec<String> {
    let tabs = accessibility::mic_tabs(pid);
    if tabs.is_empty() {
        return vec![name];
    }
    tabs.into_iter()
        .map(|tab| format!("{name} — {tab}"))
        .collect()
}

pub fn mic_apps() -> Vec<MicApp> {
    let mut processes: Vec<(i32, String)> =
        own_process::without(mic::input_pids(), std::process::id() as i32)
            .into_iter()
            .filter_map(app_process)
            .collect();
    processes.sort();
    processes.dedup();
    processes
        .into_iter()
        .filter(|(_, name)| !metadata_index::is_excluded(name))
        .map(|(pid, name)| MicApp {
            pid,
            labels: labels(pid, name.clone()),
            name,
        })
        .collect()
}

fn users_of(apps: &[MicApp]) -> Vec<String> {
    let mut users: Vec<String> = apps.iter().flat_map(|app| app.labels.clone()).collect();
    users.sort();
    users.dedup();
    users
}

pub fn mic_users() -> Vec<String> {
    users_of(&mic_apps())
}

pub fn wake(app: &AppHandle) {
    let _ = app.state::<Sender<()>>().send(());
}

pub fn start(app: AppHandle) {
    let (waker, woken) = mpsc::channel::<()>();
    app.manage(waker);
    thread::spawn(move || {
        let mut last = (Vec::new(), false);
        let mut sessions = Sessions::default();
        loop {
            let apps = mic_apps();
            let (next, events) = mic_session::step(sessions, &apps, Instant::now());
            sessions = next;
            for event in events {
                match event {
                    Event::Start(mic_app) => {
                        let _ = app.emit("mic-session-start", &mic_app);
                        meeting::handle(&app, Input::SessionStart(mic_app));
                    }
                    Event::End(mic_app) => {
                        let _ = app.emit("mic-session-end", &mic_app);
                        meeting::handle(&app, Input::SessionEnd(mic_app.pid));
                    }
                }
            }
            meeting::handle(&app, Input::Tick);
            let current = (users_of(&apps), recording::is_active(&app));
            if let Some(tray) = app.tray_by_id("main") {
                if current.1 != last.1 {
                    let _ = tray_menu::build(&app, current.1)
                        .and_then(|menu| tray.set_menu(Some(menu)));
                }
                if current != last {
                    let _ = tray.set_title(tray_title::of(&current.0, current.1));
                }
            }
            if current.0 != last.0 {
                let _ = app.emit("mic-users", &current.0);
            }
            last = current;
            let _ = woken.recv_timeout(Duration::from_secs(1));
        }
    });
}
