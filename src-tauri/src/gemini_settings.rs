use serde::{Deserialize, Serialize};
use std::process::Command;

const SERVICE: &str = "com.mic-monitor.gemini";
const ACCOUNT: &str = "mic-monitor";

#[derive(Debug, Serialize, Deserialize)]
pub struct GeminiSettings {
    pub configured: bool,
    pub model: Option<String>,
}

fn keychain(args: &[&str]) -> Result<String, String> {
    let output = Command::new("security")
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

pub fn get() -> Result<Option<String>, String> {
    if !cfg!(target_os = "macos") {
        return Err("keychain is only available on macOS".into());
    }
    match keychain(&["find-generic-password", "-a", ACCOUNT, "-s", SERVICE, "-w"]) {
        Ok(value) if !value.is_empty() => Ok(Some(value)),
        _ => Ok(None),
    }
}

pub fn set(api_key: &str) -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("keychain is only available on macOS".into());
    }
    keychain(&[
        "add-generic-password",
        "-a",
        ACCOUNT,
        "-s",
        SERVICE,
        "-w",
        api_key,
        "-U",
    ])
    .map(|_| ())
}

pub fn clear() -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("keychain is only available on macOS".into());
    }
    keychain(&["delete-generic-password", "-a", ACCOUNT, "-s", SERVICE]).map(|_| ())
}
