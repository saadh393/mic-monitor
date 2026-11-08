# Mic Monitor

A macOS menu bar app that notices when a meeting starts and offers to record it.

When any app starts using the microphone, a panel slides down from the notch asking whether to record. On Yes, Mic Monitor records your microphone and the meeting app's audio into one `.m4a` file.

## Features

- Detects any app that starts using the microphone, including the browser tab for Chrome meetings
- Notch panel prompt with Yes / No, treated as No after 15 seconds
- Records the default microphone plus only the meeting app's audio (Core Audio process tap)
- Saves to `~/Documents/Mic Monitor/<YYYY-MM-DD HH-mm> <App>.m4a`
- Stops when the meeting app releases the mic for 5 seconds, or from Stop in the menu bar or window
- Status window with the current recording and a list of past recordings: open, Show in Finder, rename, move to Trash
- Starts at login, lives in the menu bar only, and saves the recording if you quit mid-meeting

## Requirements

- macOS 14.2 or later
- [Rust](https://rustup.rs) (stable)
- [Node.js](https://nodejs.org) 18 or later

## Build

```bash
npm install
npm run tauri build
```

The app is written to `src-tauri/target/release/bundle/macos/Mic Monitor.app`. To also get a disk image:

```bash
npm run tauri build -- --bundles app,dmg
```

Run in development with `npm run tauri dev`, and the tests with `cargo test` in `src-tauri/`.

## Permissions

| Permission | Used for |
| --- | --- |
| Microphone | Recording your voice |
| Accessibility | Showing browser tab names |
| System Audio Recording | Recording the meeting app's audio |

If a permission is denied, the panel says what is missing and offers Open Settings. Unsigned local builds may lose Accessibility and System Audio Recording after each rebuild.

## Notes

- Without headphones the microphone also picks up the speakers, so remote voices can appear twice
- Recording others may require their consent where you live

## Project layout

- `src-tauri/src/` — Rust backend, one module per file
- `ui/` — status window (`index.html`) and notch panel (`panel.html`)
