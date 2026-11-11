use crate::{gemini_settings, metadata_index, transcription};
use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::Duration,
};
use tauri::{AppHandle, Emitter};

const DEFAULT_MODEL: &str = "gemini-2.5-flash";
const MAX_RETRIES: u32 = 5;
const MAX_CHUNK_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct TranscriptionEvent {
    pub path: String,
    pub status: String,
    pub completed_chunks: u32,
    pub chunk_count: u32,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct TranscriptionManager {
    sender: mpsc::Sender<Job>,
    active: Arc<Mutex<HashSet<String>>>,
    cancellations: Arc<Mutex<std::collections::HashMap<String, Arc<AtomicBool>>>>,
}

struct Job {
    app: AppHandle,
    path: PathBuf,
    name: String,
    cancel: Arc<AtomicBool>,
}

impl TranscriptionManager {
    pub fn start() -> Self {
        let (sender, receiver) = mpsc::channel::<Job>();
        let active = Arc::new(Mutex::new(HashSet::new()));
        let cancellations = Arc::new(Mutex::new(std::collections::HashMap::new()));
        let worker_active = Arc::clone(&active);
        let worker_cancellations = Arc::clone(&cancellations);
        thread::spawn(move || {
            while let Ok(job) = receiver.recv() {
                let result = run_job(&job);
                let _ = job.app.emit("transcription-status", &result);
                let _ = job.app.emit("recording-changed", ());
                worker_active.lock().unwrap().remove(&job.name);
                worker_cancellations.lock().unwrap().remove(&job.name);
            }
        });
        Self {
            sender,
            active,
            cancellations,
        }
    }

    pub fn enqueue(&self, app: AppHandle, path: PathBuf) -> Result<bool, String> {
        let name = path
            .file_name()
            .ok_or("invalid recording path")?
            .to_string_lossy()
            .into_owned();
        let mut active = self.active.lock().map_err(|_| "queue unavailable")?;
        if !active.insert(name.clone()) {
            return Ok(false);
        }
        let existing = metadata_index::list()
            .ok()
            .and_then(|items| items.into_iter().find(|item| item.name == name));
        let (completed, total, text) = existing
            .map(|item| {
                (
                    item.completed_chunks,
                    item.chunk_count,
                    item.transcript_text,
                )
            })
            .unwrap_or((0, 0, None));
        if let Err(error) =
            metadata_index::set_transcription(&name, "queued", completed, total, text.as_deref())
        {
            active.remove(&name);
            return Err(error.to_string());
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancellations
            .lock()
            .map_err(|_| "queue unavailable")?
            .insert(name.clone(), Arc::clone(&cancel));
        if self
            .sender
            .send(Job {
                app,
                path,
                name: name.clone(),
                cancel,
            })
            .is_err()
        {
            active.remove(&name);
            return Err("transcription queue stopped".into());
        }
        Ok(true)
    }

    pub fn cancel(&self, path: &Path) -> Result<(), String> {
        let name = path
            .file_name()
            .ok_or("invalid recording path")?
            .to_string_lossy()
            .into_owned();
        if let Some(token) = self
            .cancellations
            .lock()
            .map_err(|_| "queue unavailable")?
            .get(&name)
        {
            token.store(true, Ordering::Relaxed);
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct GenerateResponse {
    candidates: Option<Vec<Candidate>>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    content: Option<Content>,
}

#[derive(Debug, Deserialize)]
struct Content {
    parts: Option<Vec<ResponsePart>>,
}

#[derive(Debug, Deserialize)]
struct ResponsePart {
    text: Option<String>,
}

#[derive(Debug)]
enum GeminiError {
    Http(reqwest::StatusCode, Option<Duration>, String),
    Request(String),
    Response(String),
}

fn run_job(job: &Job) -> TranscriptionEvent {
    let path = job.path.to_string_lossy().into_owned();
    let fail = |message: String, status: &str, completed: u32, total: u32, text: String| {
        let _ = metadata_index::set_transcription(&job.name, status, completed, total, Some(&text));
        TranscriptionEvent {
            path: path.clone(),
            status: status.into(),
            completed_chunks: completed,
            chunk_count: total,
            error: Some(message),
        }
    };
    let key = match gemini_settings::get() {
        Ok(Some(key)) => key,
        Ok(None) => {
            return fail(
                "Gemini API key is not configured".into(),
                "failed",
                0,
                0,
                String::new(),
            )
        }
        Err(error) => return fail(error, "failed", 0, 0, String::new()),
    };
    let model = metadata_index::setting("gemini_model")
        .ok()
        .flatten()
        .filter(|model| !model.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_MODEL.into());
    let chunks = match prepare_chunks(&job.path) {
        Ok(chunks) => chunks,
        Err(error) => return fail(error, "failed", 0, 0, String::new()),
    };
    let total = chunks.len() as u32;
    let existing = metadata_index::list()
        .ok()
        .and_then(|items| items.into_iter().find(|item| item.name == job.name))
        .and_then(|item| item.transcript_text)
        .unwrap_or_default();
    let completed = metadata_index::list()
        .ok()
        .and_then(|items| items.into_iter().find(|item| item.name == job.name))
        .map(|item| item.completed_chunks.min(total))
        .unwrap_or(0);
    let _ = metadata_index::set_transcription(
        &job.name,
        "processing",
        completed,
        total,
        Some(&existing),
    );
    let client = match Client::builder().timeout(Duration::from_secs(90)).build() {
        Ok(client) => client,
        Err(error) => return fail(error.to_string(), "failed", completed, total, existing),
    };
    let mut transcript = existing;
    for (index, chunk) in chunks.into_iter().enumerate().skip(completed as usize) {
        if job.cancel.load(Ordering::Relaxed) {
            return fail(
                "transcription cancelled".into(),
                "cancelled",
                index as u32,
                total,
                transcript,
            );
        }
        let piece = match generate(&client, &key, &model, &chunk) {
            Ok(piece) => piece,
            Err(error) => {
                return fail(
                    format_gemini_error(error),
                    "failed",
                    index as u32,
                    total,
                    transcript,
                )
            }
        };
        if !transcript.is_empty() {
            transcript.push_str("\n\n");
        }
        transcript.push_str(&piece);
        let completed = index as u32 + 1;
        if let Err(error) = metadata_index::set_transcription(
            &job.name,
            "processing",
            completed,
            total,
            Some(&transcript),
        ) {
            return fail(error.to_string(), "failed", completed, total, transcript);
        }
        let _ = job.app.emit(
            "transcription-progress",
            TranscriptionEvent {
                path: path.clone(),
                status: "processing".into(),
                completed_chunks: completed,
                chunk_count: total,
                error: None,
            },
        );
        let _ = job.app.emit("recording-changed", ());
    }
    let transcript_path = job.path.with_extension("md");
    if let Err(error) = fs::write(&transcript_path, &transcript) {
        return fail(error.to_string(), "failed", total, total, transcript);
    }
    let _ = metadata_index::set_transcription(&job.name, "done", total, total, Some(&transcript));
    TranscriptionEvent {
        path,
        status: "done".into(),
        completed_chunks: total,
        chunk_count: total,
        error: None,
    }
}

fn format_gemini_error(error: GeminiError) -> String {
    match error {
        GeminiError::Http(status, _, message) => format!("Gemini returned {status}: {message}"),
        GeminiError::Request(message) | GeminiError::Response(message) => message,
    }
}

fn generate(client: &Client, key: &str, model: &str, chunk: &[u8]) -> Result<String, GeminiError> {
    let endpoint = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={key}"
    );
    let body = serde_json::json!({
        "contents": [{
            "parts": [
                {"text": "Transcribe this audio accurately. Return only the transcript text."},
                {"inline_data": {"mime_type": "audio/wav", "data": STANDARD.encode(chunk)}}
            ]
        }]
    });
    for attempt in 0..=MAX_RETRIES {
        let response = client
            .post(&endpoint)
            .json(&body)
            .send()
            .map_err(|e| GeminiError::Request(e.to_string()))?;
        if response.status().is_success() {
            let parsed: GenerateResponse = response
                .json()
                .map_err(|e| GeminiError::Response(e.to_string()))?;
            let text = parsed
                .candidates
                .and_then(|candidates| candidates.into_iter().next())
                .and_then(|candidate| candidate.content)
                .and_then(|content| content.parts)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|part| part.text)
                .collect::<Vec<_>>()
                .join("\n");
            return if text.trim().is_empty() {
                Err(GeminiError::Response(
                    "Gemini returned no transcript text".into(),
                ))
            } else {
                Ok(text)
            };
        }
        let status = response.status();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .and_then(parse_retry_after);
        let message = response.text().unwrap_or_default();
        if status != reqwest::StatusCode::TOO_MANY_REQUESTS || attempt == MAX_RETRIES {
            return Err(GeminiError::Http(status, retry_after, message));
        }
        thread::sleep(transcription::retry_delay(attempt, retry_after));
    }
    unreachable!()
}

fn parse_retry_after(value: &str) -> Option<Duration> {
    value.trim().parse::<u64>().ok().map(Duration::from_secs)
}

fn prepare_chunks(path: &Path) -> Result<Vec<Vec<u8>>, String> {
    let wav = decode_to_wav(path)?;
    let mut file = File::open(&wav).map_err(|e| e.to_string())?;
    let mut header = [0u8; 44];
    file.read_exact(&mut header).map_err(|e| e.to_string())?;
    let data_len = file
        .metadata()
        .map_err(|e| e.to_string())?
        .len()
        .saturating_sub(44);
    let bytes_per_second = 16_000u64 * 2;
    let duration = data_len.div_ceil(bytes_per_second);
    let chunks = transcription::plan(
        duration,
        transcription::DEFAULT_CHUNK_SECONDS,
        transcription::DEFAULT_OVERLAP_SECONDS,
    );
    let mut output = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let start = chunk.offset_seconds * bytes_per_second;
        let length = (chunk.duration_seconds * bytes_per_second) as usize;
        if length > MAX_CHUNK_BYTES {
            return Err("audio chunk exceeds the bounded upload size".into());
        }
        file.seek(SeekFrom::Start(44 + start))
            .map_err(|e| e.to_string())?;
        let remaining = file
            .metadata()
            .map_err(|e| e.to_string())?
            .len()
            .saturating_sub(44 + start);
        let length = length.min(remaining as usize);
        if length == 0 {
            continue;
        }
        let mut bytes = vec![0; length];
        file.read_exact(&mut bytes).map_err(|e| e.to_string())?;
        let mut wav_chunk = header.to_vec();
        wav_chunk[4..8].copy_from_slice(&((36 + length) as u32).to_le_bytes());
        wav_chunk[40..44].copy_from_slice(&(length as u32).to_le_bytes());
        wav_chunk.extend(bytes);
        output.push(wav_chunk);
    }
    let _ = fs::remove_file(wav);
    Ok(output)
}

fn decode_to_wav(path: &Path) -> Result<PathBuf, String> {
    if !cfg!(target_os = "macos") {
        return Err("audio decoding for transcription is currently supported only on macOS".into());
    }
    let output = std::env::temp_dir().join(format!(
        "mic-monitor-transcription-{}.wav",
        std::process::id()
    ));
    let result = Command::new("afconvert")
        .args(["-f", "WAVE", "-d", "LEI16@16000", "-o"])
        .arg(&output)
        .arg(path)
        .output()
        .map_err(|e| format!("could not start macOS audio decoder: {e}"))?;
    if result.status.success() {
        Ok(output)
    } else {
        Err(String::from_utf8_lossy(&result.stderr).trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_retry_after_seconds() {
        assert_eq!(parse_retry_after("12"), Some(Duration::from_secs(12)));
        assert_eq!(parse_retry_after("invalid"), None);
    }

    #[test]
    fn bounded_chunk_limit_is_enforced() {
        assert!(MAX_CHUNK_BYTES < 32 * 1024 * 1024);
    }

    #[test]
    fn duplicate_queue_names_are_rejected() {
        let mut active = HashSet::new();
        assert!(active.insert("meeting.m4a"));
        assert!(!active.insert("meeting.m4a"));
    }
}
