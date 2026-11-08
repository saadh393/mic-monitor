use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::{CFType, TCFType};
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplicationActivationOptions, NSMainMenuWindowLevel, NSRunningApplication, NSScreen, NSWindow, NSWindowCollectionBehavior, NSWorkspace};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering::SeqCst};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

use crate::active_screen;
use crate::panel_frame::{self, Rect};

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGWindowListCopyWindowInfo(option: u32, relative_to_window: u32) -> CFArrayRef;
    fn CGRectMakeWithDictionaryRepresentation(dict: CFDictionaryRef, rect: *mut NSRect) -> bool;
}

const ON_SCREEN_ONLY: u32 = 1 << 0;
const EXCLUDE_DESKTOP_ELEMENTS: u32 = 1 << 4;
const LABEL: &str = "panel";
const WIDTH: f64 = 380.0;
const HEIGHT: f64 = 72.0;
const SLIDE: Duration = Duration::from_millis(350);
pub const MESSAGE_TIMEOUT: Duration = Duration::from_secs(5);

static PREVIOUS: AtomicI32 = AtomicI32::new(0);
static GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, serde::Serialize)]
pub struct Payload {
    pub kind: &'static str,
    pub text: String,
    pub path: Option<String>,
}

type WindowInfo = CFDictionary<CFString, CFType>;

fn rect(r: NSRect) -> Rect {
    Rect { x: r.origin.x, y: r.origin.y, w: r.size.width, h: r.size.height }
}

fn number(info: &WindowInfo, key: &str) -> Option<i64> {
    info.find(CFString::new(key))?.downcast::<CFNumber>()?.to_i64()
}

fn bounds(info: &WindowInfo) -> Option<NSRect> {
    let value = info.find(CFString::new("kCGWindowBounds"))?;
    let mut bounds = NSRect::ZERO;
    unsafe { CGRectMakeWithDictionaryRepresentation(value.as_CFTypeRef() as CFDictionaryRef, &mut bounds) }.then_some(bounds)
}

fn front_window(pid: i32) -> Option<NSRect> {
    let list = unsafe { CGWindowListCopyWindowInfo(ON_SCREEN_ONLY | EXCLUDE_DESKTOP_ELEMENTS, 0) };
    if list.is_null() {
        return None;
    }
    let windows: CFArray<WindowInfo> = unsafe { CFArray::wrap_under_create_rule(list) };
    let found = windows.iter().find(|info| number(info, "kCGWindowLayer") == Some(0) && number(info, "kCGWindowOwnerPID") == Some(pid as i64)).and_then(|info| bounds(&info));
    found
}

fn to_appkit(window: NSRect, primary_height: f64) -> Rect {
    Rect { x: window.origin.x, y: primary_height - window.origin.y - window.size.height, w: window.size.width, h: window.size.height }
}

fn frontmost_pid() -> Option<i32> {
    NSWorkspace::sharedWorkspace().frontmostApplication().map(|app| app.processIdentifier())
}

fn place(window: &NSWindow, pid: Option<i32>, mtm: MainThreadMarker) {
    let screens: Vec<_> = NSScreen::screens(mtm).iter().collect();
    let Some(primary) = screens.first() else { return };
    let frames: Vec<Rect> = screens.iter().map(|screen| rect(screen.frame())).collect();
    let main = NSScreen::mainScreen(mtm).map(|screen| rect(screen.frame()));
    let fallback = frames.iter().position(|frame| Some(*frame) == main).unwrap_or(0);
    let active = pid.and_then(front_window).map(|bounds| to_appkit(bounds, primary.frame().size.height));
    let screen = &screens[active_screen::pick(&frames, active, fallback)];
    let panel = panel_frame::frame(rect(screen.frame()), rect(screen.visibleFrame()), screen.safeAreaInsets().top, WIDTH, HEIGHT);
    window.setFrame_display(NSRect::new(NSPoint::new(panel.x, panel.y), NSSize::new(panel.w, panel.h)), true);
}

pub fn show(app: &AppHandle, payload: Payload) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let (Some(mtm), Some(panel)) = (MainThreadMarker::new(), handle.get_webview_window(LABEL)) else { return };
        let Ok(ns_window) = panel.ns_window() else { return };
        let window: &NSWindow = unsafe { &*ns_window.cast() };
        let generation = GENERATION.fetch_add(1, SeqCst) + 1;
        let pid = frontmost_pid();
        if let Some(pid) = pid {
            PREVIOUS.store(pid, SeqCst);
        }
        place(window, pid, mtm);
        window.setLevel(NSMainMenuWindowLevel + 2);
        window.setCollectionBehavior(NSWindowCollectionBehavior::CanJoinAllSpaces | NSWindowCollectionBehavior::FullScreenAuxiliary | NSWindowCollectionBehavior::Stationary | NSWindowCollectionBehavior::IgnoresCycle);
        window.orderFrontRegardless();
        if payload.kind != "prompt" {
            hide_after(handle.clone(), generation);
        }
        let _ = handle.emit_to(LABEL, "panel-show", payload);
    });
}

fn hide_after(app: AppHandle, generation: u64) {
    thread::spawn(move || {
        thread::sleep(MESSAGE_TIMEOUT);
        if GENERATION.load(SeqCst) == generation {
            hide(&app);
        }
    });
}

fn reactivate(pid: i32) {
    if frontmost_pid() == Some(pid) {
        return;
    }
    if let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) {
        app.activateWithOptions(NSApplicationActivationOptions::empty());
    }
}

pub fn hide(app: &AppHandle) {
    let _ = app.emit_to(LABEL, "panel-hide", ());
    let generation = GENERATION.load(SeqCst);
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || reactivate(PREVIOUS.load(SeqCst)));
    thread::spawn(move || {
        thread::sleep(SLIDE);
        let window = handle.clone();
        let _ = handle.run_on_main_thread(move || {
            if GENERATION.load(SeqCst) == generation {
                if let Some(panel) = window.get_webview_window(LABEL) {
                    let _ = panel.hide();
                }
            }
        });
    });
}
