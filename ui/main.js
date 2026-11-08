const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const apps = document.getElementById("apps");
const empty = document.getElementById("empty");
const permission = document.getElementById("permission");
const accessibility = document.getElementById("accessibility");

function renderApps(names) {
  apps.replaceChildren(
    ...names.map((name) => Object.assign(document.createElement("li"), { textContent: name })),
  );
  empty.hidden = names.length > 0;
}

function renderPermission(status) {
  permission.hidden = status !== "denied";
}

function renderAccessibility(trusted) {
  accessibility.hidden = trusted;
}

function refreshPermission() {
  invoke("permission_status").then(renderPermission);
  invoke("accessibility_trusted").then(renderAccessibility);
}

function openSettings(id, pane) {
  document.getElementById(id).addEventListener("click", () => invoke("open_privacy_settings", { pane }));
}

openSettings("open-microphone-settings", "Privacy_Microphone");
openSettings("open-accessibility-settings", "Privacy_Accessibility");
window.addEventListener("focus", refreshPermission);
listen("mic-users", (event) => renderApps(event.payload));
listen("permission", (event) => renderPermission(event.payload));
invoke("mic_users").then(renderApps);
refreshPermission();

const recordingRow = document.getElementById("recording");
const recordingText = document.getElementById("recording-text");
const recordings = document.getElementById("recordings");
const recordingsStatus = document.getElementById("recordings-status");
const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
let current = null;
let refreshes = 0;

const pad = (n) => String(n).padStart(2, "0");

function elapsed(started) {
  const seconds = Math.max(0, Math.floor((Date.now() - started) / 1000));
  return `${pad(Math.floor(seconds / 60))}:${pad(seconds % 60)}`;
}

function date(ms) {
  const d = new Date(ms);
  return `${d.getDate()} ${months[d.getMonth()]} ${d.getFullYear()} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function size(bytes) {
  return bytes < 1e6 ? `${Math.max(1, Math.round(bytes / 1e3))} KB` : `${(bytes / 1e6).toFixed(1)} MB`;
}

function tick() {
  if (current) recordingText.textContent = `Recording ${current.app} — ${elapsed(current.started)}`;
}

function renderCurrent(recording) {
  current = recording;
  recordingRow.hidden = !recording;
  tick();
}

function button(text, onClick) {
  const element = Object.assign(document.createElement("button"), { textContent: text });
  element.addEventListener("click", onClick);
  return element;
}

function act(command, path) {
  invoke(command, { path }).then(refreshRecordings);
}

const messages = {
  not_found: "Recording not found",
  exists: "A recording with that name already exists",
  empty: "Enter a name",
  invalid_character: "Names can't contain / or :",
  leading_dot: "Names can't start with a dot",
};
let open = null;

function close() {
  open = null;
  refreshRecordings();
}

function failed(note, fallback) {
  return (error) => {
    const shown = open;
    note.textContent = messages[error] ?? fallback;
    if (error === "not_found") setTimeout(() => open === shown && close(), 3000);
  };
}

function edit(li, item, ...parts) {
  if (open && open.li !== li) normal(open.li, open.item);
  open = { li, item };
  li.replaceChildren(...parts);
}

function confirmDelete(li, item) {
  const note = Object.assign(document.createElement("span"), { className: "note" });
  const question = Object.assign(document.createElement("span"), { className: "question", textContent: `Move ${item.name} to the Trash?` });
  const info = Object.assign(document.createElement("div"), { className: "info" });
  info.append(question, note);
  const trash = () => invoke("delete_recording", { path: item.path }).then(close, failed(note, "Could not move to the Trash"));
  edit(li, item, info, button("Move to Trash", trash), button("Cancel", close));
}

function rename(li, item) {
  const note = Object.assign(document.createElement("span"), { className: "note" });
  const input = Object.assign(document.createElement("input"), { value: item.name.replace(/\.m4a$/, "") });
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") invoke("rename_recording", { path: item.path, name: input.value }).then(close, failed(note, "Could not rename"));
  });
  const info = Object.assign(document.createElement("div"), { className: "info" });
  info.append(input, note);
  edit(li, item, info, button("Cancel", close));
  input.focus();
  input.select();
}

function normal(li, item) {
  const name = button(item.name, () => act("open_recording", item.path));
  name.className = "name";
  name.title = item.name;
  const meta = Object.assign(document.createElement("span"), { className: "meta", textContent: `${date(item.modified)} · ${size(item.size)}` });
  const info = Object.assign(document.createElement("div"), { className: "info" });
  info.append(name, meta);
  li.replaceChildren(
    info,
    button("Show in Finder", () => act("show_in_finder", item.path)),
    button("Rename", () => rename(li, item)),
    button("Delete", () => confirmDelete(li, item)),
  );
}

function row(item) {
  const li = document.createElement("li");
  normal(li, item);
  return li;
}

function renderRecordings(items) {
  if (open) return;
  recordings.replaceChildren(...items.map(row));
  recordingsStatus.textContent = "No recordings yet";
  recordingsStatus.hidden = items.length > 0;
}

function renderRecordingsError() {
  if (open) return;
  recordings.replaceChildren();
  recordingsStatus.textContent = "Recordings could not be read";
  recordingsStatus.hidden = false;
}

function refreshRecordings() {
  const refresh = ++refreshes;
  const latest = (render) => (value) => refresh === refreshes && render(value);
  invoke("current_recording").then(latest(renderCurrent));
  invoke("recordings").then(latest(renderRecordings), latest(renderRecordingsError));
}

document.getElementById("stop").addEventListener("click", () => invoke("stop_recording"));
window.addEventListener("focus", refreshRecordings);
listen("recording-changed", refreshRecordings);
document.addEventListener("keydown", (event) => event.key === "Escape" && open && close());
setInterval(tick, 250);
refreshRecordings();
