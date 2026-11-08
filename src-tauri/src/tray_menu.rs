use tauri::menu::{Menu, MenuItem};
use tauri::{AppHandle, Wry};

pub fn build(app: &AppHandle, recording: bool) -> tauri::Result<Menu<Wry>> {
    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(app, "show", "Show", true, None::<&str>)?)?;
    if recording {
        menu.append(&MenuItem::with_id(app, "stop", "Stop", true, None::<&str>)?)?;
    }
    menu.append(&MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?)?;
    Ok(menu)
}
