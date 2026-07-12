#[cfg(target_os = "macos")]
use cocoa::base::{id, nil};
#[cfg(target_os = "macos")]
use cocoa::foundation::{NSAutoreleasePool, NSString};
#[cfg(target_os = "macos")]
use core_foundation::base::TCFType;
#[cfg(target_os = "macos")]
use core_foundation::boolean::CFBoolean;
#[cfg(target_os = "macos")]
use core_foundation::dictionary::CFDictionary;
#[cfg(target_os = "macos")]
use core_foundation::string::CFString;
use log::{error, info};
#[cfg(target_os = "macos")]
use objc::declare::ClassDecl;
#[cfg(target_os = "macos")]
use objc::runtime::{Class, Object, Sel};
#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};
#[cfg(target_os = "macos")]
use std::collections::{HashMap, HashSet};
#[cfg(target_os = "macos")]
use std::ffi::c_void;
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "macos")]
use std::sync::Mutex;
#[cfg(target_os = "macos")]
use std::sync::{Arc, OnceLock};
#[cfg(target_os = "macos")]
use std::thread;
#[cfg(target_os = "macos")]
use std::time::Duration;

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
    fn CGEventCreateKeyboardEvent(
        source: *const c_void,
        virtual_key: u16,
        key_down: bool,
    ) -> *const c_void;
    fn CGEventPost(tap_location: u32, event: *const c_void);
    fn CGEventSetFlags(event: *const c_void, flags: u64);
    fn CFRelease(cf: *const c_void);
}

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFPreferencesAppSynchronize(application_id: *const c_void) -> bool;
    fn CFPreferencesCopyAppValue(
        key: *const c_void,
        application_id: *const c_void,
    ) -> *const c_void;
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn CGWindowListCopyWindowInfo(option: u32, relativeToWindow: u32) -> *const c_void;
    static kCGWindowOwnerName: *const c_void;
    static kCGWindowOwnerPID: *const c_void;
    static kCGWindowLayer: *const c_void;
}

// Globals to keep observer and callback alive and allow cleanup
#[cfg(target_os = "macos")]
static CALLBACK: OnceLock<Arc<dyn Fn() + Send + Sync>> = OnceLock::new();
#[cfg(target_os = "macos")]
static OBSERVER: OnceLock<usize> = OnceLock::new();
#[cfg(target_os = "macos")]
static NOTIF_CENTER: OnceLock<usize> = OnceLock::new();
#[cfg(target_os = "macos")]
static FP_TO_INDEX: OnceLock<Mutex<HashMap<u64, u32>>> = OnceLock::new();
#[cfg(target_os = "macos")]
static PROMPT_REQUESTED: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "macos")]
static SETTINGS_PANE_OPENED: AtomicBool = AtomicBool::new(false);

// Sleep/Wake observers and callbacks
#[cfg(target_os = "macos")]
static SLEEP_CALLBACK_WILL: OnceLock<Arc<dyn Fn() + Send + Sync>> = OnceLock::new();
#[cfg(target_os = "macos")]
static SLEEP_CALLBACK_WAKE: OnceLock<Arc<dyn Fn() + Send + Sync>> = OnceLock::new();
#[cfg(target_os = "macos")]
static SLEEP_OBSERVER: OnceLock<usize> = OnceLock::new();
#[cfg(target_os = "macos")]
static SLEEP_NOTIF_CENTER: OnceLock<usize> = OnceLock::new();

#[cfg(target_os = "macos")]
pub fn is_accessibility_enabled() -> bool {
    unsafe { AXIsProcessTrustedWithOptions(std::ptr::null()) }
}

#[cfg(target_os = "macos")]
pub fn prompt_accessibility_permissions() -> bool {
    unsafe {
        if PROMPT_REQUESTED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            let key = CFString::from_static_string("AXTrustedCheckOptionPrompt");
            let value = CFBoolean::true_value();
            let options = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), value.as_CFType())]);
            AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef() as *const c_void)
        } else {
            let trusted = AXIsProcessTrustedWithOptions(std::ptr::null());
            // The system dialog is shown at most once per app; if the user
            // dismissed it, the only remaining path is System Settings.
            // Open the Accessibility pane for them, once per process.
            if !trusted
                && SETTINGS_PANE_OPENED
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
            {
                open_accessibility_settings();
            }
            trusted
        }
    }
}

#[cfg(target_os = "macos")]
pub fn open_accessibility_settings() {
    let _ = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();
}

/// Virtual key codes of the digits 1-9, indexed by `space_index - 1`.
#[cfg(target_os = "macos")]
const DIGIT_KEY_CODES: [u16; 9] = [18, 19, 20, 21, 23, 22, 26, 28, 25];

/// Control modifier as it appears both in CGEvent flags and in the
/// `AppleSymbolicHotKeys` parameter triple.
#[cfg(target_os = "macos")]
const CONTROL_FLAG: u64 = 0x40000;

/// macOS numbers "Switch to Desktop 1".."Switch to Desktop 9" as symbolic
/// hotkeys 118..126.
#[cfg(target_os = "macos")]
const DESKTOP_HOTKEY_BASE: u32 = 118;

#[cfg(target_os = "macos")]
const SYMBOLIC_HOTKEYS_DOMAIN: &str = "com.apple.symbolichotkeys";

/// One entry of the `AppleSymbolicHotKeys` preference.
#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymbolicHotkey {
    pub enabled: bool,
    pub key_code: i64,
    pub modifiers: u64,
}

/// Spaces that `switch_to_space` can actually reach: the "Switch to Desktop N"
/// hotkey must be enabled *and* still bound to the Ctrl+N combination we
/// synthesize. A hotkey remapped to another combination is reported as
/// unavailable, because our injected Ctrl+N would never trigger it.
#[cfg(target_os = "macos")]
fn available_desktop_indices(hotkeys: &HashMap<u32, SymbolicHotkey>) -> Vec<u32> {
    (1..=9)
        .filter(|index| {
            hotkeys
                .get(&(DESKTOP_HOTKEY_BASE + index - 1))
                .is_some_and(|hotkey| {
                    hotkey.enabled
                        && hotkey.key_code == i64::from(DIGIT_KEY_CODES[(index - 1) as usize])
                        && hotkey.modifiers == CONTROL_FLAG
                })
        })
        .collect()
}

#[cfg(target_os = "macos")]
unsafe fn object_for_key(dict: id, key: &str) -> id {
    let key = CFString::new(key);
    msg_send![dict, objectForKey: key.as_concrete_TypeRef() as id]
}

/// Reads one `AppleSymbolicHotKeys` entry: `{ enabled, value: { parameters:
/// (ascii, key code, modifiers) } }`.
#[cfg(target_os = "macos")]
unsafe fn parse_symbolic_hotkey(entry: id) -> Option<SymbolicHotkey> {
    let enabled_obj = object_for_key(entry, "enabled");
    let value_obj = object_for_key(entry, "value");
    if enabled_obj == nil || value_obj == nil {
        return None;
    }
    let parameters = object_for_key(value_obj, "parameters");
    if parameters == nil {
        return None;
    }
    let count: usize = msg_send![parameters, count];
    if count < 3 {
        return None;
    }

    let enabled: i64 = msg_send![enabled_obj, integerValue];
    let key_code_obj: id = msg_send![parameters, objectAtIndex: 1u64];
    let modifiers_obj: id = msg_send![parameters, objectAtIndex: 2u64];
    let key_code: i64 = msg_send![key_code_obj, integerValue];
    let modifiers: i64 = msg_send![modifiers_obj, integerValue];

    Some(SymbolicHotkey {
        enabled: enabled != 0,
        key_code,
        modifiers: modifiers.max(0) as u64,
    })
}

#[cfg(target_os = "macos")]
fn read_desktop_hotkeys() -> HashMap<u32, SymbolicHotkey> {
    let mut hotkeys = HashMap::new();
    unsafe {
        let _pool = NSAutoreleasePool::new(nil);
        let domain = CFString::new(SYMBOLIC_HOTKEYS_DOMAIN);
        let domain_ref = domain.as_concrete_TypeRef() as *const c_void;
        // System Settings writes this domain through cfprefsd, so our cached
        // copy goes stale the moment the user toggles a shortcut. Re-sync
        // before reading, otherwise the status never flips while Limen runs.
        CFPreferencesAppSynchronize(domain_ref);

        let key = CFString::new("AppleSymbolicHotKeys");
        let value =
            CFPreferencesCopyAppValue(key.as_concrete_TypeRef() as *const c_void, domain_ref);
        if value.is_null() {
            return hotkeys;
        }

        let entries: id = value as id;
        for index in 1..=9u32 {
            let hotkey_id = DESKTOP_HOTKEY_BASE + index - 1;
            let entry = object_for_key(entries, &hotkey_id.to_string());
            if entry == nil {
                continue;
            }
            if let Some(hotkey) = parse_symbolic_hotkey(entry) {
                hotkeys.insert(hotkey_id, hotkey);
            }
        }
        CFRelease(value);
    }
    hotkeys
}

/// Spaces that `switch_to_space` can currently reach. Empty means Space
/// switching is inert: the injected Ctrl+N lands on nothing.
#[cfg(target_os = "macos")]
pub fn available_space_shortcuts() -> Vec<u32> {
    available_desktop_indices(&read_desktop_hotkeys())
}

#[cfg(target_os = "macos")]
pub fn open_keyboard_shortcut_settings() {
    let _ = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.keyboard?Shortcuts")
        .spawn();
}

#[cfg(target_os = "macos")]
pub fn switch_to_space(space_index: u32) -> Result<(), String> {
    if !is_accessibility_enabled() {
        error!(
            "switch_to_space({}): Accessibility not granted",
            space_index
        );
        let _ = prompt_accessibility_permissions();
        return Err("Accessibility permission not granted".to_string());
    }

    unsafe {
        let _pool = NSAutoreleasePool::new(nil);
        info!("switch_to_space({}) invoked", space_index);

        if !(1..=9).contains(&space_index) {
            error!(
                "switch_to_space: index {} out of supported range (1-9)",
                space_index
            );
            return Err(format!("Space index {} out of range (1-9)", space_index));
        }

        info!("switch_to_space: using direct Ctrl+{}", space_index);
        let key_code = DIGIT_KEY_CODES[(space_index - 1) as usize];

        // Create and post key down event
        let key_down = CGEventCreateKeyboardEvent(std::ptr::null(), key_code, true);
        if key_down.is_null() {
            return Err("Failed to create key down event".to_string());
        }
        CGEventSetFlags(key_down, CONTROL_FLAG);
        CGEventPost(0, key_down);
        CFRelease(key_down);

        // Small delay
        thread::sleep(Duration::from_millis(50));

        // Create and post key up event
        let key_up = CGEventCreateKeyboardEvent(std::ptr::null(), key_code, false);
        if key_up.is_null() {
            return Err("Failed to create key up event".to_string());
        }
        CGEventSetFlags(key_up, CONTROL_FLAG);
        CGEventPost(0, key_up);
        CFRelease(key_up);

        info!(
            "switch_to_space: completed direct switch to {}",
            space_index
        );
        // Learn mapping fingerprint -> index for future passive detection.
        // Runs on a worker thread: it waits for the window list to settle
        // and must not delay the switch response to the caller.
        thread::spawn(move || record_current_space_index(space_index));
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn hash_mix(mut h: u64, v: u64) -> u64 {
    h ^= v.wrapping_mul(0x9E3779B185EBCA87);
    h = h.rotate_left(27);
    h
}

#[cfg(target_os = "macos")]
fn screen_fingerprint(limit: usize) -> u64 {
    #[allow(unused)]
    let start = std::time::Instant::now();
    const K_ONSCREEN_ONLY: u32 = 1; // kCGWindowListOptionOnScreenOnly
    const K_EXCLUDE_DESKTOP: u32 = 16; // kCGWindowListExcludeDesktopElements
    let mut h: u64 = 0xCBF29CE484222325;
    unsafe {
        // Ensure autoreleased Cocoa objects from the window list walk
        // are drained even when invoked off the main thread.
        let _pool = NSAutoreleasePool::new(nil);
        let list = CGWindowListCopyWindowInfo(K_ONSCREEN_ONLY | K_EXCLUDE_DESKTOP, 0);
        if list.is_null() {
            return h;
        }
        let windows: id = list as id;
        let count: usize = msg_send![windows, count];
        for i in 0..count.min(limit) {
            let dict: id = msg_send![windows, objectAtIndex: i as u64];
            if dict == nil {
                continue;
            }
            let layer_obj: id = msg_send![dict, objectForKey: kCGWindowLayer];
            let mut layer_val: i64 = 0;
            if layer_obj != nil {
                layer_val = msg_send![layer_obj, integerValue];
            }
            if layer_val != 0 {
                continue;
            }

            let pid_obj: id = msg_send![dict, objectForKey: kCGWindowOwnerPID];
            let mut pid_val: i32 = 0;
            if pid_obj != nil {
                pid_val = msg_send![pid_obj, intValue];
            }

            let owner_name_obj: id = msg_send![dict, objectForKey: kCGWindowOwnerName];
            let name = if owner_name_obj != nil {
                nsstring_to_string(owner_name_obj)
            } else {
                String::new()
            };
            let mut local: u64 = 0;
            for b in name.as_bytes() {
                local = local.wrapping_mul(131).wrapping_add(*b as u64);
            }
            local = hash_mix(local, (pid_val as i64 as i128 as u128) as u64);

            h = hash_mix(h, local ^ (layer_val as i128 as u128) as u64);
        }
        CFRelease(list);
    }
    crate::perf::record(
        "screen_fingerprint",
        serde_json::json!({ "limit": limit, "duration_ms": crate::perf::elapsed_ms(start) }),
    );
    h
}

#[cfg(target_os = "macos")]
fn fp_map_cell() -> &'static Mutex<HashMap<u64, u32>> {
    FP_TO_INDEX.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(target_os = "macos")]
fn current_space_fingerprint() -> u64 {
    // Hash the top few visible layer-0 windows; stable enough for switching inference
    screen_fingerprint(6)
}

#[cfg(target_os = "macos")]
pub fn record_current_space_index(index: u32) {
    // Wait briefly so window list stabilizes after a switch
    thread::sleep(Duration::from_millis(180));
    let fp = current_space_fingerprint();
    if let Ok(mut map) = fp_map_cell().lock() {
        map.insert(fp, index);
    }
}

#[cfg(target_os = "macos")]
pub fn infer_active_space_index() -> Option<u32> {
    let fp = current_space_fingerprint();
    fp_map_cell().lock().ok().and_then(|m| m.get(&fp).copied())
}

#[cfg(target_os = "macos")]
pub fn fp_map_size() -> usize {
    fp_map_cell().lock().ok().map(|m| m.len()).unwrap_or(0)
}

#[cfg(all(test, target_os = "macos"))]
pub fn insert_fingerprint_for_test(fp: u64, idx: u32) {
    let _ = fp_map_cell().lock().map(|mut m| m.insert(fp, idx));
}

#[cfg(all(test, target_os = "macos"))]
pub fn lookup_fingerprint_for_test(fp: u64) -> Option<u32> {
    fp_map_cell().lock().ok().and_then(|m| m.get(&fp).copied())
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    fn ctrl_digit(index: u32) -> SymbolicHotkey {
        SymbolicHotkey {
            enabled: true,
            key_code: i64::from(DIGIT_KEY_CODES[(index - 1) as usize]),
            modifiers: CONTROL_FLAG,
        }
    }

    fn hotkeys(entries: &[(u32, SymbolicHotkey)]) -> HashMap<u32, SymbolicHotkey> {
        entries.iter().copied().collect()
    }

    #[test]
    // 仕様: Ctrl+数字に割り当てられ有効な Desktop だけを利用可能として返す
    fn available_desktops_are_enabled_and_bound_to_control_digit() {
        let map = hotkeys(&[
            (118, ctrl_digit(1)),
            (119, ctrl_digit(2)),
            (126, ctrl_digit(9)),
        ]);
        assert_eq!(available_desktop_indices(&map), vec![1, 2, 9]);
    }

    #[test]
    // 仕様: エントリが存在しない Desktop は利用不可(macOS の既定は無効)
    fn missing_entry_is_unavailable() {
        assert_eq!(
            available_desktop_indices(&HashMap::new()),
            Vec::<u32>::new()
        );
    }

    #[test]
    // 仕様: enabled = false の Desktop は利用不可
    fn disabled_hotkey_is_unavailable() {
        let map = hotkeys(&[(
            118,
            SymbolicHotkey {
                enabled: false,
                ..ctrl_digit(1)
            },
        )]);
        assert_eq!(available_desktop_indices(&map), Vec::<u32>::new());
    }

    #[test]
    // 仕様: 別の修飾キーに割り当て直された Desktop は利用不可(注入する Ctrl+N が届かない)
    fn remapped_modifier_is_unavailable() {
        let map = hotkeys(&[(
            118,
            SymbolicHotkey {
                modifiers: CONTROL_FLAG | 0x80000, // Ctrl+Option
                ..ctrl_digit(1)
            },
        )]);
        assert_eq!(available_desktop_indices(&map), Vec::<u32>::new());
    }

    #[test]
    // 仕様: 別のキーに割り当て直された Desktop は利用不可
    fn remapped_key_code_is_unavailable() {
        let map = hotkeys(&[(
            118,
            SymbolicHotkey {
                key_code: 42,
                ..ctrl_digit(1)
            },
        )]);
        assert_eq!(available_desktop_indices(&map), Vec::<u32>::new());
    }

    #[test]
    // 仕様: 118..126 以外の symbolic hotkey は Desktop 切り替えと無関係
    fn unrelated_hotkeys_are_ignored() {
        let map = hotkeys(&[(79, ctrl_digit(1)), (127, ctrl_digit(1))]);
        assert_eq!(available_desktop_indices(&map), Vec::<u32>::new());
    }

    #[test]
    // 仕様: 指紋→インデックスの挿入と取得ができる
    fn fp_map_insert_and_lookup() {
        insert_fingerprint_for_test(12345, 2);
        assert_eq!(lookup_fingerprint_for_test(12345), Some(2));
    }

    #[test]
    // 仕様: 同一指紋への再挿入は上書きされ、最新のインデックスが返る
    fn fp_map_overwrite() {
        insert_fingerprint_for_test(999, 3);
        insert_fingerprint_for_test(999, 7);
        assert_eq!(lookup_fingerprint_for_test(999), Some(7));
    }
}

#[cfg(target_os = "macos")]
pub fn setup_space_change_listener<F>(callback: F) -> Result<(), String>
where
    F: Fn() + Send + Sync + 'static,
{
    unsafe {
        let _pool = NSAutoreleasePool::new(nil);

        // Get NSWorkspace class
        let workspace_class = Class::get("NSWorkspace").ok_or("Failed to get NSWorkspace")?;
        let shared_workspace: id = msg_send![workspace_class, sharedWorkspace];

        if shared_workspace == nil {
            return Err("Failed to get shared workspace".to_string());
        }

        // Get notification center
        let notification_center: id = msg_send![shared_workspace, notificationCenter];
        if notification_center == nil {
            return Err("Failed to get notification center".to_string());
        }

        // Selector target: call Rust callback stored in OnceLock
        extern "C" fn space_changed(_this: &Object, _cmd: Sel, _notification: id) {
            if let Some(cb) = CALLBACK.get() {
                cb();
            }
        }

        // Register a tiny Objective-C class with a selector to receive notifications
        let observer_class_name = "LimenSpaceObserver";
        if Class::get(observer_class_name).is_none() {
            let nsobject = Class::get("NSObject").ok_or("Failed to get NSObject class")?;
            let mut decl = ClassDecl::new(observer_class_name, nsobject)
                .ok_or("Failed to declare observer class")?;
            decl.add_method(
                sel!(spaceChanged:),
                space_changed as extern "C" fn(&Object, Sel, id),
            );
            decl.register();
        }

        let observer_class = Class::get(observer_class_name).ok_or("Missing observer class")?;
        let observer: id = msg_send![observer_class, new];
        if observer == nil {
            return Err("Failed to create observer instance".to_string());
        }

        // Store the callback and keep strong ref to observer and center
        let _ = CALLBACK.set(Arc::new(callback));
        let _ = OBSERVER.set(observer as usize);
        let _ = NOTIF_CENTER.set(notification_center as usize);

        // Subscribe to NSWorkspaceActiveSpaceDidChangeNotification
        let name: id = NSString::alloc(nil).init_str("NSWorkspaceActiveSpaceDidChangeNotification");
        let _: () = msg_send![
            notification_center,
            addObserver: observer
            selector: sel!(spaceChanged:)
            name: name
            object: nil
        ];

        Ok(())
    }
}

#[cfg(target_os = "macos")]
pub fn remove_space_change_listener() {
    unsafe {
        if let (Some(observer_ptr), Some(center_ptr)) = (OBSERVER.get(), NOTIF_CENTER.get()) {
            let observer: id = *observer_ptr as id;
            let center: id = *center_ptr as id;
            let _: () = msg_send![center, removeObserver:observer];
            let _: () = msg_send![observer, release];
        }
    }
}

#[cfg(target_os = "macos")]
pub fn setup_sleep_wake_listeners<F1, F2>(on_will_sleep: F1, on_did_wake: F2) -> Result<(), String>
where
    F1: Fn() + Send + Sync + 'static,
    F2: Fn() + Send + Sync + 'static,
{
    unsafe {
        let _pool = NSAutoreleasePool::new(nil);

        let workspace_class = Class::get("NSWorkspace").ok_or("Failed to get NSWorkspace")?;
        let shared_workspace: id = msg_send![workspace_class, sharedWorkspace];
        if shared_workspace == nil {
            return Err("Failed to get shared workspace".to_string());
        }

        let notification_center: id = msg_send![shared_workspace, notificationCenter];
        if notification_center == nil {
            return Err("Failed to get notification center".to_string());
        }

        extern "C" fn will_sleep(_this: &Object, _cmd: Sel, _notification: id) {
            if let Some(cb) = SLEEP_CALLBACK_WILL.get() {
                cb();
            }
        }

        extern "C" fn did_wake(_this: &Object, _cmd: Sel, _notification: id) {
            if let Some(cb) = SLEEP_CALLBACK_WAKE.get() {
                cb();
            }
        }

        let observer_class_name = "LimenSleepObserver";
        if Class::get(observer_class_name).is_none() {
            let nsobject = Class::get("NSObject").ok_or("Failed to get NSObject class")?;
            let mut decl = ClassDecl::new(observer_class_name, nsobject)
                .ok_or("Failed to declare sleep observer class")?;
            decl.add_method(
                sel!(willSleep:),
                will_sleep as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(sel!(didWake:), did_wake as extern "C" fn(&Object, Sel, id));
            decl.register();
        }

        let observer_class =
            Class::get(observer_class_name).ok_or("Missing sleep observer class")?;
        let observer: id = msg_send![observer_class, new];
        if observer == nil {
            return Err("Failed to create sleep observer instance".to_string());
        }

        let _ = SLEEP_CALLBACK_WILL.set(Arc::new(on_will_sleep));
        let _ = SLEEP_CALLBACK_WAKE.set(Arc::new(on_did_wake));
        let _ = SLEEP_OBSERVER.set(observer as usize);
        let _ = SLEEP_NOTIF_CENTER.set(notification_center as usize);

        let will_name: id = NSString::alloc(nil).init_str("NSWorkspaceWillSleepNotification");
        let did_name: id = NSString::alloc(nil).init_str("NSWorkspaceDidWakeNotification");

        let _: () = msg_send![
            notification_center,
            addObserver: observer
            selector: sel!(willSleep:)
            name: will_name
            object: nil
        ];
        let _: () = msg_send![
            notification_center,
            addObserver: observer
            selector: sel!(didWake:)
            name: did_name
            object: nil
        ];

        Ok(())
    }
}

#[cfg(target_os = "macos")]
pub fn remove_sleep_wake_listeners() {
    unsafe {
        if let (Some(observer_ptr), Some(center_ptr)) =
            (SLEEP_OBSERVER.get(), SLEEP_NOTIF_CENTER.get())
        {
            let observer: id = *observer_ptr as id;
            let center: id = *center_ptr as id;
            let _: () = msg_send![center, removeObserver:observer];
            let _: () = msg_send![observer, release];
        }
    }
}

#[cfg(target_os = "macos")]
pub fn frontmost_app_info() -> Option<(String, String)> {
    unsafe {
        let _pool = NSAutoreleasePool::new(nil);

        let workspace_class = Class::get("NSWorkspace")?;
        let shared_workspace: id = msg_send![workspace_class, sharedWorkspace];
        if shared_workspace == nil {
            return None;
        }

        let app: id = msg_send![shared_workspace, frontmostApplication];
        if app == nil {
            return None;
        }

        // Helpers to convert NSString* to Rust String
        unsafe fn nsstring_to_string(s: id) -> String {
            let cstr: *const std::os::raw::c_char = msg_send![s, UTF8String];
            if cstr.is_null() {
                String::new()
            } else {
                std::ffi::CStr::from_ptr(cstr)
                    .to_string_lossy()
                    .into_owned()
            }
        }

        // bundleIdentifier
        let bundle_id_ns: id = msg_send![app, bundleIdentifier];
        if bundle_id_ns == nil {
            return None;
        }
        let bundle_id = nsstring_to_string(bundle_id_ns);

        // localizedName
        let name_ns: id = msg_send![app, localizedName];
        let name = if name_ns != nil {
            nsstring_to_string(name_ns)
        } else {
            bundle_id.clone()
        };

        // Filter out system/background apps from frontmost as well
        if is_system_or_background(&bundle_id, None) {
            return None;
        }

        Some((bundle_id, name))
    }
}

#[cfg(target_os = "macos")]
unsafe fn nsstring_to_string(s: id) -> String {
    let cstr: *const std::os::raw::c_char = msg_send![s, UTF8String];
    if cstr.is_null() {
        String::new()
    } else {
        std::ffi::CStr::from_ptr(cstr)
            .to_string_lossy()
            .into_owned()
    }
}

#[cfg(target_os = "macos")]
pub fn collect_visible_apps(limit: usize) -> Vec<crate::AppInfo> {
    const K_ONSCREEN_ONLY: u32 = 1; // kCGWindowListOptionOnScreenOnly
    const K_EXCLUDE_DESKTOP: u32 = 16; // kCGWindowListExcludeDesktopElements

    #[allow(unused)]
    let start = std::time::Instant::now();

    let mut result: Vec<crate::AppInfo> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    unsafe {
        // Important: this function is called from a background std::thread.
        // Many Cocoa APIs return autoreleased objects. Without an
        // NSAutoreleasePool on this thread, those objects would leak until
        // thread exit, leading to gradual memory growth and eventual stalls
        // after long uptime or sleep/wake cycles. Ensure we drain them.
        let _pool = NSAutoreleasePool::new(nil);

        let list = CGWindowListCopyWindowInfo(K_ONSCREEN_ONLY | K_EXCLUDE_DESKTOP, 0);
        if list.is_null() {
            #[cfg(feature = "profiling")]
            log::info!(
                "profiling::collect_visible_apps limit={} windows=0 apps=0 elapsed_ms={}",
                limit,
                start.elapsed().as_millis()
            );
            return result;
        }
        let windows: id = list as id; // toll-free bridged CFArrayRef -> NSArray*
        let count: usize = msg_send![windows, count];

        let running_cls = Class::get("NSRunningApplication").unwrap();
        let workspace_cls = Class::get("NSWorkspace").unwrap();
        let shared_ws: id = msg_send![workspace_cls, sharedWorkspace];

        for i in 0..count {
            if result.len() >= limit {
                break;
            }
            let dict: id = msg_send![windows, objectAtIndex: i as u64];
            if dict == nil {
                continue;
            }

            // layer == 0 (normal windows)のみ
            let layer_obj: id = msg_send![dict, objectForKey: kCGWindowLayer];
            if layer_obj != nil {
                let layer: i64 = msg_send![layer_obj, integerValue];
                if layer != 0 {
                    continue;
                }
            }

            let pid_obj: id = msg_send![dict, objectForKey: kCGWindowOwnerPID];
            if pid_obj == nil {
                continue;
            }
            let pid: i32 = msg_send![pid_obj, intValue];

            let app: id = msg_send![running_cls, runningApplicationWithProcessIdentifier: pid];
            if app == nil {
                continue;
            }

            let hidden: bool = msg_send![app, isHidden];
            if hidden {
                continue;
            }

            let bundle_id_ns: id = msg_send![app, bundleIdentifier];
            if bundle_id_ns == nil {
                continue;
            }
            let bundle_id = nsstring_to_string(bundle_id_ns);
            if bundle_id.is_empty() || seen.contains(&bundle_id) {
                continue;
            }

            // Determine owner name (e.g., Dock, Window Server) for filtering
            let mut owner_name: Option<String> = None;
            let owner_obj: id = msg_send![dict, objectForKey: kCGWindowOwnerName];
            if owner_obj != nil {
                let s = nsstring_to_string(owner_obj);
                if !s.is_empty() {
                    owner_name = Some(s);
                }
            }

            // Skip system/background apps
            if is_system_or_background(&bundle_id, owner_name.as_deref()) {
                continue;
            }

            let name_ns: id = msg_send![app, localizedName];
            let name = if name_ns != nil {
                nsstring_to_string(name_ns)
            } else {
                bundle_id.clone()
            };

            // Try to get icon (base64 PNG)
            let mut icon_b64: Option<String> = None;
            let bundle_url: id = msg_send![app, bundleURL];
            if bundle_url != nil {
                let path_ns: id = msg_send![bundle_url, path];
                if path_ns != nil {
                    let img: id = msg_send![shared_ws, iconForFile: path_ns];
                    if img != nil {
                        let tiff: id = msg_send![img, TIFFRepresentation];
                        if tiff != nil {
                            let rep_cls = Class::get("NSBitmapImageRep").unwrap();
                            let rep: id = msg_send![rep_cls, imageRepWithData: tiff];
                            if rep != nil {
                                // 4 = NSPNGFileType
                                let png: id =
                                    msg_send![rep, representationUsingType: 4u64 properties: nil];
                                if png != nil {
                                    let b64_ns: id =
                                        msg_send![png, base64EncodedStringWithOptions: 0u64];
                                    if b64_ns != nil {
                                        icon_b64 = Some(nsstring_to_string(b64_ns));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            result.push(crate::AppInfo {
                bundle_id: bundle_id.clone(),
                name,
                icon_b64,
            });
            seen.insert(bundle_id);
        }

        CFRelease(list);
    }

    crate::perf::record(
        "collect_visible_apps",
        serde_json::json!({
            "limit": limit,
            "windows_scanned": seen.len(),
            "apps_found": result.len(),
            "duration_ms": crate::perf::elapsed_ms(start),
        }),
    );

    result
}

#[cfg(target_os = "macos")]
fn is_system_or_background(bundle_id: &str, owner_name: Option<&str>) -> bool {
    let bid = bundle_id.to_ascii_lowercase();
    let owner = owner_name.unwrap_or("").to_ascii_lowercase();

    // Explicit bundle id blocklist
    let blocked_ids: &[&str] = &[
        // Core system/agents
        "com.apple.dock",
        "com.apple.windowserver",
        "com.apple.systemuiserver",
        "com.apple.notificationcenterui",
        "com.apple.notificationcenter",
        "com.apple.loginwindow",
        "com.apple.controlcenter",
        "com.apple.spindump",
        "com.apple.reportcrash",
        "com.apple.screensaver.engine",
        // Background utilities
        "com.apple.spotlight",
    ];

    if blocked_ids.iter().any(|p| bid == *p) {
        return true;
    }

    // Heuristics: background-style identifiers
    let blocked_suffixes = ["agent", "daemon", "service", "helper", "ui"];
    if bid.starts_with("com.apple.") && blocked_suffixes.iter().any(|s| bid.ends_with(s)) {
        return true;
    }

    // Owner name hints
    if !owner.is_empty() {
        let owner_block = [
            "dock",
            "window server",
            "notification center",
            "control center",
        ];
        if owner_block.iter().any(|s| owner.contains(s)) {
            return true;
        }
    }

    false
}
