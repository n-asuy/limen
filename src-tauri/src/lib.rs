use log::{error, info};
use serde::{Deserialize, Serialize};
use std::fs::File as StdFile;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

mod config;
#[cfg(target_os = "macos")]
mod macos;
mod perf;
mod persistence;
mod tray;

static ACTIVE_SPACE_ID: OnceLock<Mutex<Option<u32>>> = OnceLock::new();
static APP_CONFIG: OnceLock<Mutex<config::Config>> = OnceLock::new();

#[derive(Clone)]
struct ShortcutRuntimeState {
    gate: Arc<AtomicBool>,
    toggle: Arc<dyn Fn() + Send + Sync>,
}

static SHORTCUT_RUNTIME: OnceLock<ShortcutRuntimeState> = OnceLock::new();
static SHORTCUT_SUSPEND_DEPTH: OnceLock<Mutex<u32>> = OnceLock::new();

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppInfo {
    pub bundle_id: String,
    pub name: String,
    pub icon_b64: Option<String>,
}

fn active_space_cell() -> &'static Mutex<Option<u32>> {
    ACTIVE_SPACE_ID.get_or_init(|| Mutex::new(None))
}

fn config_cell() -> &'static Mutex<config::Config> {
    APP_CONFIG.get_or_init(|| Mutex::new(config::Config::load()))
}

fn shortcut_runtime() -> &'static ShortcutRuntimeState {
    SHORTCUT_RUNTIME
        .get()
        .expect("shortcut runtime not initialized")
}

fn shortcut_suspend_depth() -> &'static Mutex<u32> {
    SHORTCUT_SUSPEND_DEPTH.get_or_init(|| Mutex::new(0))
}

fn resolve_tray_digit_icon_path(
    app: &tauri::AppHandle,
    digit: Option<u32>,
) -> Option<std::path::PathBuf> {
    let name = match digit {
        Some(d) if (1..=9).contains(&d) => format!("{}.png", d),
        _ => "dot.png".to_string(),
    };

    let rel = std::path::PathBuf::from("icons")
        .join("tray_digits")
        .join(&name);

    // 1) Packaged app: Resources/icons/tray_digits/<name>
    if let Ok(res_dir) = app.path().resource_dir() {
        let p = res_dir.join(&rel);
        if p.exists() {
            return Some(p);
        }
    }
    // 2) Dev: relative to src-tauri working dir
    if rel.exists() {
        return Some(rel);
    }
    None
}

fn update_tray_icon(app: &tauri::AppHandle) {
    #[cfg(desktop)]
    {
        if let Some(tray) = app.tray_by_id("main-tray") {
            let idx_opt = active_space_cell().lock().ok().and_then(|g| *g);
            // 1) Preferred: user-provided digit PNG
            if let Some(p) = resolve_tray_digit_icon_path(app, idx_opt) {
                if let Ok(img) = tauri::image::Image::from_path(&p) {
                    let _ = tray.set_icon(Some(img));
                    let _ = tray.set_icon_as_template(true);
                    let _ = tray.set_title::<&str>(None);
                    return;
                }
            }

            // 2) Fallback: show dot.png if available
            if let Some(p) = resolve_tray_digit_icon_path(app, None) {
                if let Ok(img) = tauri::image::Image::from_path(&p) {
                    let _ = tray.set_icon(Some(img));
                    let _ = tray.set_icon_as_template(true);
                    let _ = tray.set_title::<&str>(None);
                    return;
                }
            }

            // 3) Last resort: no icon and no title (user images should exist; avoid debug dot)
            let _ = tray.set_icon(None);
            let _ = tray.set_icon_as_template(false);
            let _ = tray.set_title::<&str>(None);
        }
    }
}

pub(crate) fn toggle_main_window(app: &tauri::AppHandle) {
    let start = std::time::Instant::now();

    // The ring stays reachable while settings is open: a user who has just
    // bound a shortcut expects it to fire right away, and settings offers a
    // button to preview the ring. Recording a new binding is already safe
    // because the recorder unregisters the global shortcuts while it listens.
    if let Some(win) = app.get_webview_window("main") {
        let visible = win.is_visible().unwrap_or(true);
        if visible {
            let _ = win.hide();
            perf::record(
                "toggle_window",
                serde_json::json!({ "action": "hide", "duration_ms": perf::elapsed_ms(start) }),
            );
        } else {
            let t_show = std::time::Instant::now();
            let _ = win.show();
            let show_ms = perf::elapsed_ms(t_show);

            let t_top = std::time::Instant::now();
            let _ = win.set_always_on_top(true);
            let top_ms = perf::elapsed_ms(t_top);

            let t_focus = std::time::Instant::now();
            let _ = win.set_focus();
            let focus_ms = perf::elapsed_ms(t_focus);

            // Notify frontend so it can start the show→hover latency clock
            let _ = app.emit("perf:window-shown", ());

            perf::record(
                "toggle_window",
                serde_json::json!({
                    "action": "show",
                    "show_ms": show_ms,
                    "always_on_top_ms": top_ms,
                    "focus_ms": focus_ms,
                    "total_ms": perf::elapsed_ms(start),
                }),
            );
        }
    }
}

fn apply_shortcuts(app: &tauri::AppHandle, combos: &[String]) -> Result<(), String> {
    let runtime = shortcut_runtime();
    let manager = app.global_shortcut();

    manager
        .unregister_all()
        .map_err(|e| format!("failed to clear shortcuts: {e}"))?;

    if combos.is_empty() {
        info!("no shortcut bindings configured; global toggle disabled");
        return Ok(());
    }

    for combo in combos {
        let trimmed = combo.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Validate again defensively before registering
        config::ShortcutPreferences::validate(trimmed)?;

        let gate = runtime.gate.clone();
        let toggle = runtime.toggle.clone();
        let label = trimmed.to_string();

        manager
            .on_shortcut(trimmed, move |_handle, _shortcut, _event| {
                let was_open = !gate.swap(false, Ordering::SeqCst);
                if !was_open {
                    let t0 = std::time::Instant::now();
                    (toggle)();
                    perf::record(
                        "shortcut_handler",
                        serde_json::json!({
                            "toggle_ms": perf::elapsed_ms(t0),
                        }),
                    );
                    let gate_clone = gate.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(300));
                        gate_clone.store(true, Ordering::SeqCst);
                    });
                } else {
                    perf::record("shortcut_handler", serde_json::json!({ "debounced": true }));
                }
            })
            .map_err(|e| format!("failed to register shortcut {label}: {e}"))?;

        info!("registered global shortcut: {label}");
    }

    Ok(())
}

pub(crate) fn data_root() -> Result<PathBuf, String> {
    dirs::data_dir()
        .map(|p| p.join("Limen"))
        .ok_or_else(|| "application data directory not available".to_string())
}

fn state_file_path() -> Result<PathBuf, String> {
    let path = data_root()?.join("space.json");
    Ok(path)
}

fn icons_dir_path() -> Result<PathBuf, String> {
    Ok(data_root()?.join("icons"))
}

#[tauri::command]
fn get_icons_dir() -> Result<String, String> {
    icons_dir_path().map(|p| p.to_string_lossy().to_string())
}

/// Decide whether the settings window should open at launch. It doubles as
/// onboarding: shown once for new users, and again whenever a prerequisite of
/// Space switching is missing, i.e. Accessibility (e.g. after a signed update
/// reset the TCC grant) or every Mission Control Desktop shortcut being off.
/// Partial shortcut coverage is left alone: switching still works, and a user
/// running four Desktops should not be nagged on every launch.
fn should_open_settings_on_launch(
    onboarded: bool,
    accessibility_granted: bool,
    space_switching_available: bool,
) -> bool {
    !onboarded || !accessibility_granted || !space_switching_available
}

pub(crate) fn ensure_settings_window(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide();
    }

    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.show();
        let _ = win.set_always_on_top(true);
        let _ = win.set_focus();
        let _ = win.set_always_on_top(false);
        return Ok(());
    }

    // Keep the native frame (traffic lights, rounded corners, drop shadow).
    // Overlay, not Transparent: the web content has to extend all the way under
    // the title bar, or the bare window background shows through above it.
    let builder = tauri::WebviewWindowBuilder::new(
        app,
        "settings",
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("Limen Settings")
    .inner_size(560.0, 640.0)
    .resizable(false)
    .maximizable(false)
    .title_bar_style(tauri::TitleBarStyle::Overlay)
    .hidden_title(true)
    .center()
    .build();

    match builder {
        Ok(win) => {
            let _ = win.set_always_on_top(true);
            let _ = win.set_focus();
            let _ = win.set_always_on_top(false);
            Ok(())
        }
        Err(err) => Err(err.to_string()),
    }
}

#[tauri::command]
fn load_state_file() -> Result<String, String> {
    let path = state_file_path()?;
    // Corrupt files are moved aside and reported as missing so the
    // frontend can re-initialize without destroying user data.
    persistence::read_json_or_quarantine(&path)
}

#[tauri::command]
fn save_state_file(app: tauri::AppHandle, content: String) -> Result<(), String> {
    let path = state_file_path()?;
    persistence::write_atomically(&path, content.as_bytes())?;
    // Space names may have changed; refresh the tray menu labels.
    tray::rebuild_menu(&app);
    Ok(())
}

#[tauri::command]
fn quarantine_state_file() -> Result<Option<String>, String> {
    let path = state_file_path()?;
    if !path.exists() {
        return Ok(None);
    }
    persistence::quarantine(&path).map(|p| Some(p.to_string_lossy().to_string()))
}

#[tauri::command]
fn toggle_window(app: tauri::AppHandle) -> Result<(), String> {
    toggle_main_window(&app);
    if app.get_webview_window("main").is_some() {
        Ok(())
    } else {
        Err("main window not found".into())
    }
}

#[tauri::command]
fn hide_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
        Ok(())
    } else {
        Err("main window not found".into())
    }
}

#[tauri::command]
fn get_active_space_index() -> Option<u32> {
    active_space_cell().lock().ok().and_then(|g| *g)
}

#[tauri::command]
fn check_accessibility() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::is_accessibility_enabled()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

#[tauri::command]
fn request_accessibility() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::prompt_accessibility_permissions()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Spaces whose Mission Control shortcut is enabled and still bound to Ctrl+N,
/// i.e. the ones `switch_space` can actually reach.
#[tauri::command]
fn check_space_shortcuts() -> Vec<u32> {
    #[cfg(target_os = "macos")]
    {
        macos::available_space_shortcuts()
    }
    #[cfg(not(target_os = "macos"))]
    {
        (1..=9).collect()
    }
}

#[tauri::command]
fn open_space_shortcut_settings() {
    #[cfg(target_os = "macos")]
    {
        macos::open_keyboard_shortcut_settings();
    }
}

#[tauri::command]
fn switch_space(app: tauri::AppHandle, index: u32) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        info!("tauri cmd: switch_space({})", index);
        let res = macos::switch_to_space(index);
        if res.is_ok() {
            if let Ok(mut guard) = active_space_cell().lock() {
                *guard = Some(index);
            }
            update_tray_icon(&app);
            info!("tauri cmd: switch_space -> ok (active={})", index);
        } else if let Err(ref e) = res {
            error!("tauri cmd: switch_space -> err: {}", e);
        }
        res
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Ok(mut guard) = active_space_cell().lock() {
            *guard = Some(index);
        }
        update_tray_icon(&app);
        Ok(())
    }
}

#[tauri::command]
fn get_shortcut_config() -> config::ShortcutPreferences {
    config_cell()
        .lock()
        .map(|cfg| cfg.shortcuts.clone())
        .unwrap_or_default()
}

/// Apply a shortcut mutation transactionally: persist first, then register.
/// Both the saved file and the registered bindings are rolled back when any
/// step fails, so config and OS state never diverge.
fn change_shortcut_config(
    app: &tauri::AppHandle,
    mutate: impl FnOnce(&mut config::ShortcutPreferences) -> Result<(), String>,
) -> Result<config::ShortcutPreferences, String> {
    let (updated, previous, bindings) = {
        let mut guard = config_cell().lock().expect("config lock poisoned");
        let previous = guard.shortcuts.clone();
        mutate(&mut guard.shortcuts)?;
        let updated = guard.shortcuts.clone();
        let bindings = updated.bindings();
        if let Err(err) = guard.save() {
            guard.shortcuts = previous;
            return Err(err);
        }
        (updated, previous, bindings)
    };

    if let Err(err) = apply_shortcuts(app, &bindings) {
        let revert_bindings = {
            let mut guard = config_cell().lock().expect("config lock poisoned");
            guard.shortcuts = previous.clone();
            if let Err(save_err) = guard.save() {
                error!("failed to restore shortcut config after error: {save_err}");
            }
            guard.shortcuts.bindings()
        };
        let _ = apply_shortcuts(app, &revert_bindings);
        return Err(err);
    }

    let _ = app.emit("shortcut-updated", &updated);
    Ok(updated)
}

#[tauri::command]
fn update_shortcut_config(
    app: tauri::AppHandle,
    primary: String,
) -> Result<config::ShortcutPreferences, String> {
    change_shortcut_config(&app, |shortcuts| shortcuts.update_primary(primary.trim()))
}

#[tauri::command]
fn reset_shortcut_config(app: tauri::AppHandle) -> Result<config::ShortcutPreferences, String> {
    change_shortcut_config(&app, |shortcuts| {
        shortcuts.reset_to_defaults();
        shortcuts.normalize();
        Ok(())
    })
}

#[tauri::command]
fn suspend_shortcuts(app: tauri::AppHandle) -> Result<(), String> {
    let mut guard = shortcut_suspend_depth()
        .lock()
        .map_err(|_| "shortcut suspend lock poisoned".to_string())?;
    if *guard == u32::MAX {
        return Err("shortcut suspend depth exceeded".to_string());
    }
    if *guard == 0 {
        app.global_shortcut()
            .unregister_all()
            .map_err(|e| format!("failed to suspend shortcuts: {e}"))?;
        info!("suspended global shortcuts for recording");
    }
    *guard += 1;
    Ok(())
}

#[tauri::command]
fn resume_shortcuts(app: tauri::AppHandle) -> Result<(), String> {
    let mut guard = shortcut_suspend_depth()
        .lock()
        .map_err(|_| "shortcut suspend lock poisoned".to_string())?;
    if *guard == 0 {
        return Ok(());
    }
    *guard -= 1;
    if *guard == 0 {
        let bindings = {
            let cfg = config_cell().lock().expect("config lock poisoned");
            cfg.shortcuts.bindings()
        };
        if let Err(err) = apply_shortcuts(&app, &bindings) {
            error!("failed to resume shortcuts: {err}");
            return Err(err);
        }
        info!("resumed global shortcuts after recording");
    }
    Ok(())
}

/// Infer the active Space, refresh the tray, and push app snapshots to the
/// frontend. Heavy (window-list walks, icon encoding, disk writes); must be
/// called from a worker thread, never the main thread.
#[cfg(target_os = "macos")]
fn refresh_space_snapshot(app_handle: &tauri::AppHandle) {
    let cb_start = std::time::Instant::now();

    // Try to infer the current space index from window fingerprinting
    let t_infer = std::time::Instant::now();
    let inferred = crate::macos::infer_active_space_index();
    let infer_ms = crate::perf::elapsed_ms(t_infer);

    if let Some(idx) = inferred {
        if let Ok(mut guard) = crate::active_space_cell().lock() {
            *guard = Some(idx);
        }
        info!("space snapshot: active index inferred = {}", idx);
        update_tray_icon(app_handle);
        // Move the ✓ marker in the tray menu to the new active Space.
        crate::tray::rebuild_menu(app_handle);
    }
    let _ = app_handle.emit("space-changed", ());

    let mut collect_ms: f64 = 0.0;
    let mut save_icons_ms: f64 = 0.0;
    let mut frontmost_ms: f64 = 0.0;

    if let Some(space_id) = active_space_cell().lock().ok().and_then(|g| *g) {
        // Visible apps snapshot
        let t_collect = std::time::Instant::now();
        let apps = crate::macos::collect_visible_apps(3);
        collect_ms = crate::perf::elapsed_ms(t_collect);

        let simple: Vec<serde_json::Value> = apps
            .iter()
            .map(|a| {
                serde_json::json!({
                    "bundle_id": a.bundle_id,
                    "name": a.name,
                })
            })
            .collect();
        let payload = serde_json::json!({
            "space_id": space_id,
            "apps": simple,
        });
        let _ = app_handle.emit("apps-visible", payload);

        let t_icons = std::time::Instant::now();
        let _ = save_visible_app_icons(Some(6));
        save_icons_ms = crate::perf::elapsed_ms(t_icons);

        // Frontmost app snapshot
        let t_front = std::time::Instant::now();
        if let Some((bundle_id, name)) = crate::macos::frontmost_app_info() {
            let payload = serde_json::json!({
                "space_id": space_id,
                "bundle_id": bundle_id,
                "name": name,
            });
            let _ = app_handle.emit("frontmost-changed", payload);
        }
        frontmost_ms = crate::perf::elapsed_ms(t_front);
    }

    crate::perf::record(
        "space_changed_callback",
        serde_json::json!({
            "infer_ms": infer_ms,
            "collect_visible_apps_ms": collect_ms,
            "save_icons_ms": save_icons_ms,
            "frontmost_ms": frontmost_ms,
            "fp_map_size": crate::macos::fp_map_size(),
            "total_ms": crate::perf::elapsed_ms(cb_start),
        }),
    );
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_log::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            toggle_window,
            hide_window,
            get_active_space_index,
            check_accessibility,
            request_accessibility,
            check_space_shortcuts,
            open_space_shortcut_settings,
            switch_space,
            load_state_file,
            save_state_file,
            quarantine_state_file,
            get_icons_dir,
            get_shortcut_config,
            update_shortcut_config,
            reset_shortcut_config,
            suspend_shortcuts,
            resume_shortcuts,
        ])
        .setup(|app| {
            // Initialise perf writer (no-op when feature = "profiling" is off)
            perf::init();

            let app_handle = app.handle().clone();
            let toggle_main: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
                toggle_main_window(&app_handle);
            });

            let gate = Arc::new(AtomicBool::new(true));
            let runtime_state = ShortcutRuntimeState {
                gate: gate.clone(),
                toggle: toggle_main.clone(),
            };
            if SHORTCUT_RUNTIME.set(runtime_state).is_err() {
                info!("shortcut runtime already initialized");
            }

            // Ensure configuration defaults are persisted and register shortcuts
            let bindings = {
                let mut guard = config_cell().lock().expect("config lock poisoned");
                guard.shortcuts.normalize();
                let bindings = guard.shortcuts.bindings();
                if let Err(err) = guard.save() {
                    error!("failed to persist config defaults: {err}");
                }
                bindings
            };

            if let Err(err) = apply_shortcuts(app.handle(), &bindings) {
                error!("failed to register shortcut bindings: {err}");
            }

            // Initialize tray (menu bar icon)
            crate::tray::init_tray(app.handle())?;
            // Set initial tray icon from prepared assets (if available)
            update_tray_icon(app.handle());

            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            #[cfg(target_os = "macos")]
            {
                // Ask for the Accessibility permission up front: the app
                // cannot switch Spaces without it, and prompting only after
                // a failed switch confuses first-run users.
                if !crate::macos::is_accessibility_enabled() {
                    let _ = crate::macos::prompt_accessibility_permissions();
                }
            }

            // First-run onboarding: open the settings window (which explains the
            // shortcut, the permission, and the Mission Control setup) on the
            // first launch, and again whenever Accessibility is missing.
            {
                let accessibility_granted = check_accessibility();
                let space_switching_available = !check_space_shortcuts().is_empty();
                let (first_run, needs_settings) = {
                    let guard = config_cell().lock().expect("config lock poisoned");
                    (
                        !guard.onboarded,
                        should_open_settings_on_launch(
                            guard.onboarded,
                            accessibility_granted,
                            space_switching_available,
                        ),
                    )
                };
                if needs_settings {
                    if let Err(err) = ensure_settings_window(app.handle()) {
                        error!("failed to open onboarding settings window: {err}");
                    }
                    if first_run {
                        let mut guard = config_cell().lock().expect("config lock poisoned");
                        guard.onboarded = true;
                        if let Err(err) = guard.save() {
                            error!("failed to persist onboarding flag: {err}");
                        }
                    }
                }
            }

            #[cfg(target_os = "macos")]
            {
                // Listen for active Space changes. The notification arrives
                // on the main thread; the snapshot work walks the window
                // list and encodes icons, so run it on a worker thread to
                // keep the UI responsive.
                let app_handle2 = app.handle().clone();
                let _ = crate::macos::setup_space_change_listener(move || {
                    let handle = app_handle2.clone();
                    std::thread::spawn(move || refresh_space_snapshot(&handle));
                });

                // Listen for system sleep/wake (for future use; currently logs only)
                let _ = crate::macos::setup_sleep_wake_listeners(
                    || {
                        info!("system will sleep");
                    },
                    || {
                        info!("system did wake");
                    },
                );

                // Emit an initial snapshot on startup
                let startup_handle = app.handle().clone();
                std::thread::spawn(move || refresh_space_snapshot(&startup_handle));
            }

            Ok(())
        });

    builder
        .run(tauri::generate_context!())
        .expect("error while running Limen");
}

/// Encode the icons of the visible apps and cache them on disk.
/// Called from the snapshot worker; heavy enough to keep off the main thread.
fn save_visible_app_icons(limit: Option<usize>) -> Result<Vec<String>, String> {
    #[cfg(target_os = "macos")]
    {
        use base64::Engine;
        let engine = base64::engine::general_purpose::STANDARD;
        let max = limit.unwrap_or(6);
        #[allow(unused)]
        let start = std::time::Instant::now();
        let apps = crate::macos::collect_visible_apps(max);
        let dir = icons_dir_path()?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut saved: Vec<String> = Vec::new();
        #[allow(unused)]
        let mut enumerated = 0usize;
        for app in apps {
            enumerated += 1;
            if let Some(ref b64) = app.icon_b64 {
                let mut name = app.bundle_id.clone();
                name = name.replace('/', "_");
                let path = dir.join(format!("{}.png", name));

                if path.exists() {
                    continue;
                }

                match engine.decode(b64.as_bytes()) {
                    Ok(bytes) => {
                        let mut f = StdFile::create(&path).map_err(|e| e.to_string())?;
                        f.write_all(&bytes).map_err(|e| e.to_string())?;
                        saved.push(path.to_string_lossy().to_string());
                    }
                    Err(err) => {
                        error!("failed to decode icon for {}: {}", app.bundle_id, err);
                    }
                }
            }
        }
        info!("saved {} app icons to {}", saved.len(), dir.display());
        perf::record(
            "save_visible_app_icons",
            serde_json::json!({
                "limit": max,
                "enumerated": enumerated,
                "saved": saved.len(),
                "duration_ms": perf::elapsed_ms(start),
            }),
        );
        Ok(saved)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::should_open_settings_on_launch;

    #[test]
    // 仕様: 未オンボーディングなら権限の有無に関わらず開く
    fn opens_for_first_run_regardless_of_permission() {
        assert!(should_open_settings_on_launch(false, false, true));
        assert!(should_open_settings_on_launch(false, true, true));
    }

    #[test]
    // 仕様: オンボーディング済みでも権限が無ければ開く(更新でTCCが失われた場合の再案内)
    fn opens_when_accessibility_missing() {
        assert!(should_open_settings_on_launch(true, false, true));
    }

    #[test]
    // 仕様: Desktop ショートカットが1つも有効でなければ開く(切り替えが全く効かない状態)
    fn opens_when_no_space_shortcut_works() {
        assert!(should_open_settings_on_launch(true, true, false));
    }

    #[test]
    // 仕様: オンボーディング済み・権限あり・切り替え可能なら開かない(通常起動)。
    // Desktop が一部だけ有効な運用(4画面など)も「可能」に含め、毎回せっつかない
    fn stays_closed_when_switching_works() {
        assert!(!should_open_settings_on_launch(true, true, true));
    }
}
