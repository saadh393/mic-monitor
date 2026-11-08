mod accessibility;
mod active_screen;
mod app_name;
mod audio_mix;
mod interleave;
mod login_item;
mod m4a_writer;
mod meeting;
mod meeting_flow;
mod meeting_processes;
mod meeting_silence;
mod mic;
mod mic_recorder;
mod mic_samples;
mod mic_session;
mod monitor;
mod own_process;
mod panel;
mod panel_frame;
mod parent_pid;
mod permission;
mod process_path;
mod process_tap;
mod recording;
mod recording_file;
mod recording_name;
mod recording_path;
mod recording_rename;
mod recordings_folder;
mod recordings_list;
mod save_error;
mod tab_title;
mod tray_menu;
mod tray_title;

use meeting_flow::{Flow, Input};
use recording::Recording;
use signal_hook::consts::SIGTERM;
use signal_hook::iterator::Signals;
use std::path::PathBuf;
use std::sync::Mutex;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::tray::TrayIconBuilder;
use tauri::{ActivationPolicy, AppHandle, Emitter, Manager, RunEvent, WindowEvent};

#[tauri::command]
fn mic_users() -> Vec<String> {
    monitor::mic_users()
}

#[tauri::command]
fn permission_status() -> &'static str {
    permission::status()
}

#[tauri::command]
fn accessibility_trusted() -> bool {
    accessibility::is_trusted()
}

#[tauri::command]
fn open_privacy_settings(pane: String) {
    let _ = std::process::Command::new("open")
        .arg(format!("x-apple.systempreferences:com.apple.preference.security?{pane}"))
        .spawn();
}

#[tauri::command]
fn show_in_finder(path: String) {
    let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
}

#[tauri::command]
fn open_recording(path: String) {
    let _ = std::process::Command::new("open").arg(path).spawn();
}

#[derive(serde::Serialize)]
struct RecordingItem {
    name: String,
    path: String,
    modified: u64,
    size: u64,
}

#[derive(serde::Serialize)]
struct CurrentRecording {
    app: String,
    started: u64,
}

fn millis(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).map(|elapsed| elapsed.as_millis() as u64).unwrap_or(0)
}

#[tauri::command]
async fn recordings(app: AppHandle) -> Result<Vec<RecordingItem>, String> {
    let folder = recordings_folder::path().ok_or("could not find the Documents folder")?;
    let entries = recordings_folder::read(&folder).map_err(|e| e.to_string())?;
    let current = recording::current(&app).map(|(name, _, _)| name);
    Ok(recordings_list::list(entries, current.as_deref())
        .into_iter()
        .map(|(name, modified, size)| RecordingItem { path: folder.join(&name).to_string_lossy().into_owned(), name, modified: millis(modified), size })
        .collect())
}

#[tauri::command]
async fn current_recording(app: AppHandle) -> Option<CurrentRecording> {
    recording::current(&app).map(|(_, app, started)| CurrentRecording { app, started: millis(started) })
}

fn guarded(app: &AppHandle, path: &str) -> Result<(PathBuf, PathBuf, String), String> {
    let folder = recordings_folder::path().ok_or("failed")?;
    let path = PathBuf::from(path);
    let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let current = recording::current(app).map(|(name, _, _)| name);
    if !recording_path::inside(&folder, &path) || current.as_deref() == Some(name.as_str()) {
        return Err("refused".into());
    }
    Ok((folder, path, name))
}

#[tauri::command]
async fn delete_recording(app: AppHandle, path: String) -> Result<(), String> {
    let (_, path, _) = guarded(&app, &path)?;
    Ok(recording_file::trash(&path)?)
}

#[tauri::command]
async fn rename_recording(app: AppHandle, path: String, name: String) -> Result<String, String> {
    let (folder, path, old) = guarded(&app, &path)?;
    let entries = recordings_folder::read(&folder).map_err(|_| "failed")?;
    let names: Vec<&str> = entries.iter().map(|(name, _, _)| name.as_str()).collect();
    if !names.contains(&old.as_str()) {
        return Err("not_found".into());
    }
    let Some(new) = recording_rename::target(&names, &old, &name).map_err(|reason| reason.code())? else {
        return Ok(path.to_string_lossy().into_owned());
    };
    let to = folder.join(new);
    recording_file::rename(&path, &to)?;
    Ok(to.to_string_lossy().into_owned())
}

#[tauri::command]
fn stop_recording(app: AppHandle) {
    meeting::handle(&app, Input::Stop);
}

#[tauri::command]
fn panel_yes(app: AppHandle) {
    meeting::handle(&app, Input::Yes);
}

#[tauri::command]
fn panel_no(app: AppHandle) {
    meeting::handle(&app, Input::No);
}

#[tauri::command]
fn panel_dismiss(app: AppHandle) {
    panel::hide(&app);
}

fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn main() {
    let mut app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_, _, _| {}))
        .invoke_handler(tauri::generate_handler![mic_users, permission_status, accessibility_trusted, open_privacy_settings, show_in_finder, open_recording, recordings, current_recording, delete_recording, rename_recording, stop_recording, panel_yes, panel_no, panel_dismiss])
        .setup(|app| {
            app.manage(Mutex::new(Flow::default()));
            app.manage(Mutex::new(None::<Recording>));
            TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&tray_menu::build(app.handle(), false)?)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => show_main(app),
                    "stop" => meeting::handle(app, Input::Stop),
                    "quit" => {
                        meeting::quit(app);
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;
            if permission::status() == "undetermined" {
                let handle = app.handle().clone();
                permission::request(move || {
                    let _ = handle.emit("permission", permission::status());
                });
            }
            if !accessibility::is_trusted() {
                accessibility::request_trust();
            }
            login_item::register();
            monitor::start(app.handle().clone());
            let handle = app.handle().clone();
            let mut signals = Signals::new([SIGTERM])?;
            thread::spawn(move || {
                if signals.forever().next().is_some() {
                    handle.exit(0);
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("failed to build Mic Monitor");
    app.set_activation_policy(ActivationPolicy::Accessory);
    app.run(|app, event| {
        if matches!(event, RunEvent::ExitRequested { .. } | RunEvent::Exit) {
            meeting::quit(app);
        }
    });
}
