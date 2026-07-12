use std::fs;
use std::path::PathBuf;
use tauri::{Emitter, Manager};

#[derive(serde::Deserialize)]
struct PersistedState {
    spaces: Vec<SpaceEntry>,
}

#[derive(serde::Deserialize)]
struct SpaceEntry {
    id: u32,
    name: String,
}

fn read_space_names() -> Vec<String> {
    // Default 9 entries, empty names
    let mut names: Vec<String> = (1..=9).map(|_| String::new()).collect();
    if let Ok(path) = crate::data_root() {
        let path: PathBuf = path.join("space.json");
        if path.exists() {
            if let Ok(txt) = fs::read_to_string(&path) {
                if let Ok(state) = serde_json::from_str::<PersistedState>(&txt) {
                    for s in state.spaces.into_iter() {
                        if (1..=9).contains(&s.id) {
                            let default_name = format!("Space {}", s.id);
                            let label = if s.name.trim() == default_name {
                                String::new()
                            } else {
                                s.name
                            };
                            let idx = (s.id - 1) as usize;
                            if idx < names.len() {
                                names[idx] = label;
                            }
                        }
                    }
                }
            }
        }
    }
    names
}

pub fn init_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    // Build menu according to requested layout
    let menu = tauri::menu::Menu::new(app)?;

    // Prepare space labels 1..9 from persisted state
    let names = read_space_names();
    #[cfg(target_os = "macos")]
    let current = crate::macos::infer_active_space_index();
    #[cfg(not(target_os = "macos"))]
    let current: Option<u32> = None;

    for i in 1..=9u32 {
        let name = names.get((i - 1) as usize).cloned().unwrap_or_default();
        let mut label = if name.trim().is_empty() {
            format!("{}", i)
        } else {
            format!("{} {}", i, name)
        };
        if current == Some(i) {
            label = format!("✓ {}", label);
        }
        let item = tauri::menu::MenuItem::with_id(
            app,
            format!("space-{}", i),
            &label,
            true,
            None::<&str>,
        )?;
        menu.append(&item)?;
        if i == 3 || i == 6 || i == 9 {
            menu.append(&tauri::menu::PredefinedMenuItem::separator(app)?)?;
        }
    }

    // Common actions
    let show_item =
        tauri::menu::MenuItem::with_id(app, "show", "Show Limen", true, None::<&str>)?;
    menu.append(&show_item)?;
    menu.append(&tauri::menu::PredefinedMenuItem::separator(app)?)?;

    // No accelerator: Limen is an Accessory app with no app menu, so a
    // displayed Cmd+, would advertise a shortcut that never fires.
    let prefs_item =
        tauri::menu::MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
    menu.append(&prefs_item)?;
    menu.append(&tauri::menu::PredefinedMenuItem::separator(app)?)?;

    let quit_item =
        tauri::menu::MenuItem::with_id(app, "quit", "Quit Limen", true, None::<&str>)?;
    menu.append(&quit_item)?;

    tauri::tray::TrayIconBuilder::with_id("main-tray")
        .tooltip("Limen")
        .show_menu_on_left_click(true)
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            id if id.starts_with("space-") => {
                // Click on numbered space item -> switch spaces (macOS)
                if let Some(idx_str) = id.strip_prefix("space-") {
                    if let Ok(idx) = idx_str.parse::<u32>() {
                        #[cfg(target_os = "macos")]
                        {
                            log::info!("tray: switching to space {} via menu", idx);
                            let _ = crate::macos::switch_to_space(idx);
                        }
                        // Emit space-changed so frontend can refresh UI
                        log::info!("tray: emitting space-changed after menu switch");
                        let _ = app.emit("space-changed", ());
                    }
                }
            }
            "show" => {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
            "settings" => {
                let _ = crate::ensure_settings_window(app);
            }
            "quit" => {
                #[cfg(target_os = "macos")]
                {
                    crate::macos::remove_space_change_listener();
                    crate::macos::remove_sleep_wake_listeners();
                }
                app.exit(0)
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // Only toggle on an actual left-click release; ignore hover/move/enter/leave
            if let tauri::tray::TrayIconEvent::Click {
                button,
                button_state,
                ..
            } = event
            {
                if button == tauri::tray::MouseButton::Left
                    && button_state == tauri::tray::MouseButtonState::Up
                {
                    let app = tray.app_handle();
                    crate::toggle_main_window(app);
                }
            }
        })
        .build(app)?;

    Ok(())
}
