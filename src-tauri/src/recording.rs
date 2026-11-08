use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};
use tauri::{AppHandle, Manager};

use crate::m4a_writer::{self, M4aWriter};
use crate::meeting_silence::{self, Check};
use crate::mic_recorder::{self, MicCapture};
use crate::mic_session::MicApp;
use crate::process_tap::{self, ProcessTap};
use crate::{audio_mix, meeting_processes, mic, monitor, panel, permission, recording_name, recordings_folder, save_error};

const CHUNK_TIMEOUT: Duration = Duration::from_millis(500);

enum Chunk {
    Mic(Vec<f32>),
    Meeting(Vec<f32>),
}

pub struct Recording {
    mic: MicCapture,
    tap: Option<ProcessTap>,
    path: PathBuf,
    app: String,
    started: SystemTime,
    writer: JoinHandle<Result<u64, String>>,
}

impl Recording {
    pub fn stop(self) -> Result<(PathBuf, u64), String> {
        self.mic.stop();
        if let Some(tap) = self.tap {
            tap.stop();
        }
        let frames = self.writer.join().map_err(|_| "the recording writer panicked".to_string())??;
        Ok((self.path, frames))
    }
}

struct Streams {
    mic: (u32, u16),
    meeting: Option<((u32, u16), Instant)>,
}

fn write_mixed(writer: &mut M4aWriter, mic: &mut Vec<f32>, meeting: &mut Vec<f32>, count: usize) -> Result<(), String> {
    let mic_part: Vec<f32> = mic.drain(..count.min(mic.len())).collect();
    let meeting_part: Vec<f32> = meeting.drain(..count.min(meeting.len())).collect();
    writer.write(&audio_mix::mix(&mic_part, &meeting_part))
}

fn write_loop(app: AppHandle, mut writer: M4aWriter, chunks: Receiver<Chunk>, streams: Streams) -> Result<u64, String> {
    let (mut mic, mut meeting) = (Vec::new(), Vec::new());
    let mut verdict = Check::Waiting;
    loop {
        let (mic_chunk, meeting_chunk) = match chunks.recv_timeout(CHUNK_TIMEOUT) {
            Ok(Chunk::Mic(samples)) => (samples, Vec::new()),
            Ok(Chunk::Meeting(samples)) => (Vec::new(), samples),
            Err(RecvTimeoutError::Timeout) => (Vec::new(), Vec::new()),
            Err(RecvTimeoutError::Disconnected) => break,
        };
        mic.extend(audio_mix::convert(&mic_chunk, streams.mic.0, streams.mic.1));
        if let Some(((rate, channels), started)) = streams.meeting {
            let next = meeting_silence::check(verdict, &meeting_chunk, started.elapsed());
            if next == Check::Missing && verdict != Check::Missing {
                panel::show(&app, panel::Payload { kind: "audio-missing", text: "Meeting audio is missing".into(), path: None });
            }
            verdict = next;
            meeting.extend(audio_mix::convert(&meeting_chunk, rate, channels));
        }
        let count = audio_mix::mixable(mic.len(), streams.meeting.map(|_| meeting.len()));
        write_mixed(&mut writer, &mut mic, &mut meeting, count)?;
    }
    let rest = mic.len().max(meeting.len());
    write_mixed(&mut writer, &mut mic, &mut meeting, rest)?;
    writer.finish()
}

fn start_tap(session: &MicApp, sender: mpsc::Sender<Chunk>) -> Option<ProcessTap> {
    let processes: Vec<(u32, String)> = mic::processes().into_iter().filter_map(|(id, pid)| Some((id, monitor::name_of(pid)?))).collect();
    match process_tap::start(meeting_processes::select(&processes, &session.name), move |samples| {
        let _ = sender.send(Chunk::Meeting(samples.to_vec()));
    }) {
        Ok(tap) => Some(tap),
        Err(e) => {
            eprintln!("could not start meeting audio capture: {e}");
            None
        }
    }
}

fn fs_error(folder: &std::path::Path, e: std::io::Error) -> (String, &'static str) {
    let kind = if save_error::is_privacy(e.raw_os_error()) { "save-denied" } else { "save-failed" };
    (format!("could not use the save folder {}: {e}", folder.display()), kind)
}

fn open_file(app_name: &str) -> Result<(PathBuf, M4aWriter), (String, &'static str)> {
    let folder = recordings_folder::path().ok_or(("could not find the Documents folder".to_string(), "save-failed"))?;
    std::fs::create_dir_all(&folder).map_err(|e| fs_error(&folder, e))?;
    let existing = std::fs::read_dir(&folder)
        .and_then(|entries| entries.map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned())).collect::<Result<Vec<_>, _>>())
        .map_err(|e| fs_error(&folder, e))?;
    let path = folder.join(recording_name::file_name(chrono::Local::now().naive_local(), app_name, &existing));
    let writer = m4a_writer::open(&path).map_err(|e| (format!("could not create {}: {e}", path.display()), "save-failed"))?;
    Ok((path, writer))
}

pub fn is_active(app: &AppHandle) -> bool {
    app.state::<Mutex<Option<Recording>>>().lock().unwrap().is_some()
}

pub fn current(app: &AppHandle) -> Option<(String, String, SystemTime)> {
    let state = app.state::<Mutex<Option<Recording>>>();
    let recording = state.lock().unwrap();
    let recording = recording.as_ref()?;
    Some((recording.path.file_name()?.to_string_lossy().into_owned(), recording.app.clone(), recording.started))
}

pub fn start(app: &AppHandle, session: &MicApp) {
    let state = app.state::<Mutex<Option<Recording>>>();
    let mut recording = state.lock().unwrap();
    if recording.is_some() {
        return;
    }
    if permission::status() != "granted" {
        panel::show(app, panel::Payload { kind: "mic-denied", text: "Microphone access is needed".into(), path: None });
        return;
    }
    let (path, writer) = match open_file(&session.name) {
        Ok(opened) => opened,
        Err((e, kind)) => {
            eprintln!("{e}");
            panel::show(app, panel::Payload { kind, text: "Recording could not be saved".into(), path: None });
            return;
        }
    };
    let (sender, chunks) = mpsc::channel();
    let mic_sender = sender.clone();
    let mic = match mic_recorder::start(move |samples| {
        let _ = mic_sender.send(Chunk::Mic(samples.to_vec()));
    }) {
        Ok(mic) => mic,
        Err(e) => {
            eprintln!("could not start microphone capture: {e}");
            drop(writer);
            let _ = std::fs::remove_file(&path);
            return;
        }
    };
    let tap = start_tap(session, sender);
    let streams = Streams { mic: (mic.rate, mic.channels), meeting: tap.as_ref().map(|tap| ((tap.rate, tap.channels), Instant::now())) };
    let handle = app.clone();
    let writer = thread::spawn(move || write_loop(handle, writer, chunks, streams));
    *recording = Some(Recording { mic, tap, path, app: session.name.clone(), started: SystemTime::now(), writer });
}
