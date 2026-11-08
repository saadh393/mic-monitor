const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const panel = document.getElementById("panel");
const text = document.getElementById("text");
const panes = {
  "mic-denied": "Privacy_Microphone",
  "audio-missing": "Privacy_ScreenCapture",
  "save-denied": "Privacy_FilesAndFolders",
};
let path = null;

listen("panel-show", (event) => {
  text.textContent = event.payload.text;
  path = event.payload.path;
  panel.dataset.kind = event.payload.kind;
  panel.classList.add("shown");
});
listen("panel-hide", () => panel.classList.remove("shown"));
document.getElementById("yes").addEventListener("click", () => invoke("panel_yes"));
document.getElementById("no").addEventListener("click", () => invoke("panel_no"));
document.getElementById("settings").addEventListener("click", async () => {
  await invoke("open_privacy_settings", { pane: panes[panel.dataset.kind] });
  invoke("panel_dismiss");
});
document.getElementById("finder").addEventListener("click", async () => {
  await invoke("show_in_finder", { path });
  invoke("panel_dismiss");
});
