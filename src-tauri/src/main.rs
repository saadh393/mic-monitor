mod accessibility;
mod active_screen;
mod app_name;
mod audio_mix;
mod gemini_settings;
mod installed_apps;
mod interleave;
mod login_item;
mod m4a_writer;
mod meeting;
mod meeting_flow;
mod meeting_processes;
mod meeting_silence;
mod metadata_index;
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
mod transcription;
mod transcription_backend;
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
        .arg(format!(
            "x-apple.systempreferences:com.apple.preference.security?{pane}"
        ))
        .spawn();
}

#[tauri::command]
fn show_in_finder(path: String) {
    let _ = std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .spawn();
}

#[tauri::command]
fn open_recording(path: String) {
    let _ = std::process::Command::new("open").arg(path).spawn();
}

#[tauri::command]
fn recording_transcript(path: String) -> Result<String, String> {
    let transcript = PathBuf::from(&path).with_extension("md");
    std::fs::read_to_string(transcript)
        .map_err(|_| "no transcript has been generated for this recording".into())
}

#[tauri::command]
fn export_transcript(path: String) -> Result<String, String> {
    let source = PathBuf::from(&path).with_extension("md");
    if !source.is_file() {
        return Err("no transcript has been generated for this recording".into());
    }
    let downloads = dirs::download_dir().ok_or("could not find the Downloads folder")?;
    let target = downloads.join(source.file_name().ok_or("invalid transcript name")?);
    std::fs::copy(&source, &target).map_err(|e| e.to_string())?;
    Ok(target.to_string_lossy().into_owned())
}

#[tauri::command]
fn transcribe_recording(
    app: AppHandle,
    manager: tauri::State<'_, transcription_backend::TranscriptionManager>,
    path: String,
) -> Result<(), String> {
    let path = PathBuf::from(path);
    path.file_name().ok_or("invalid recording path")?;
    let _ = manager.enqueue(app, path)?;
    Ok(())
}

#[tauri::command]
fn cancel_transcription(
    manager: tauri::State<'_, transcription_backend::TranscriptionManager>,
    path: String,
) -> Result<(), String> {
    manager.cancel(PathBuf::from(path).as_path())?;
    let name = PathBuf::from(path)
        .file_name()
        .ok_or("invalid recording path")?
        .to_string_lossy()
        .into_owned();
    let existing = metadata_index::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|item| item.name == name);
    let (completed, total, text) = existing
        .map(|item| {
            (
                item.completed_chunks,
                item.chunk_count,
                item.transcript_text,
            )
        })
        .unwrap_or((0, 0, None));
    metadata_index::set_transcription(&name, "cancelled", completed, total, text.as_deref())
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(serde::Serialize)]
struct CurrentRecording {
    app: String,
    started: u64,
}

fn millis(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

#[tauri::command]
async fn recordings(app: AppHandle) -> Result<Vec<metadata_index::RecordingMetadata>, String> {
    let folder = recordings_folder::path().ok_or("could not find the Documents folder")?;
    let indexed = metadata_index::reconcile(&folder).map_err(|e| e.to_string())?;
    let current = recording::current(&app).map(|(name, _, _)| name);
    Ok(indexed
        .into_iter()
        .filter(|item| current.as_deref() != Some(item.name.as_str()))
        .collect())
}

#[tauri::command]
async fn current_recording(app: AppHandle) -> Option<CurrentRecording> {
    recording::current(&app).map(|(_, app, started)| CurrentRecording {
        app,
        started: millis(started),
    })
}

fn guarded(app: &AppHandle, path: &str) -> Result<(PathBuf, PathBuf, String), String> {
    let folder = recordings_folder::path().ok_or("failed")?;
    let path = PathBuf::from(path);
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let current = recording::current(app).map(|(name, _, _)| name);
    if !recording_path::inside(&folder, &path) || current.as_deref() == Some(name.as_str()) {
        return Err("refused".into());
    }
    Ok((folder, path, name))
}

#[tauri::command]
async fn delete_recording(app: AppHandle, path: String) -> Result<(), String> {
    let (_, path, name) = guarded(&app, &path)?;
    recording_file::trash(&path)?;
    metadata_index::remove(&name).map_err(|e| e.to_string())
}

#[tauri::command]
async fn rename_recording(app: AppHandle, path: String, name: String) -> Result<String, String> {
    let (folder, path, old) = guarded(&app, &path)?;
    let entries = recordings_folder::read(&folder).map_err(|_| "failed")?;
    let names: Vec<&str> = entries.iter().map(|(name, _, _)| name.as_str()).collect();
    if !names.contains(&old.as_str()) {
        return Err("not_found".into());
    }
    let Some(new) =
        recording_rename::target(&names, &old, &name).map_err(|reason| reason.code())?
    else {
        return Ok(path.to_string_lossy().into_owned());
    };
    let to = folder.join(new);
    recording_file::rename(&path, &to)?;
    metadata_index::rename(&old, &new, &to).map_err(|e| e.to_string())?;
    Ok(to.to_string_lossy().into_owned())
}

#[tauri::command]
fn excluded_apps() -> Result<Vec<metadata_index::ExcludedApp>, String> {
    metadata_index::excluded().map_err(|e| e.to_string())
}

#[tauri::command]
fn set_excluded_app(name: String, excluded: bool) -> Result<(), String> {
    metadata_index::set_excluded(&name, excluded).map_err(|e| e.to_string())
}

#[tauri::command]
fn installed_apps() -> Vec<installed_apps::InstalledApp> {
    installed_apps::list()
}

#[tauri::command]
fn gemini_settings() -> Result<gemini_settings::GeminiSettings, String> {
    Ok(gemini_settings::GeminiSettings {
        configured: gemini_settings::get()?.is_some(),
        model: metadata_index::setting("gemini_model").map_err(|e| e.to_string())?,
    })
}

#[tauri::command]
fn save_gemini_settings(api_key: String, model: Option<String>) -> Result<(), String> {
    if api_key.trim().is_empty() {
        return Err("api_key cannot be empty".into());
    }
    gemini_settings::set(api_key.trim())?;
    if let Some(model) = model.filter(|model| !model.trim().is_empty()) {
        metadata_index::set_setting("gemini_model", model.trim()).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn clear_gemini_settings() -> Result<(), String> {
    gemini_settings::clear()
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
        .invoke_handler(tauri::generate_handler![
            mic_users,
            permission_status,
            accessibility_trusted,
            open_privacy_settings,
            show_in_finder,
            open_recording,
            recording_transcript,
            export_transcript,
            transcribe_recording,
            cancel_transcription,
            recordings,
            current_recording,
            delete_recording,
            rename_recording,
            excluded_apps,
            set_excluded_app,
            installed_apps,
            gemini_settings,
            save_gemini_settings,
            clear_gemini_settings,
            stop_recording,
            panel_yes,
            panel_no,
            panel_dismiss
        ])
        .setup(|app| {
            app.manage(transcription_backend::TranscriptionManager::start());
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
