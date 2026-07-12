use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use tauri_plugin_global_shortcut::Shortcut;

use crate::data_root;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
#[derive(Default)]
pub struct Config {
    pub shortcuts: ShortcutPreferences,
    /// True once the first-run settings window has been shown. Absent in
    /// pre-onboarding config files, which serde(default) resolves to false.
    pub onboarded: bool,
}


#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct ShortcutPreferences {
    pub primary: String,
    pub fallback: Vec<String>,
}

impl Default for ShortcutPreferences {
    fn default() -> Self {
        // Option+Space: one-handed, "Space (key) for Spaces" mnemonic, and
        // holding Option keeps the in-ring Option+digit hints active.
        #[cfg(target_os = "macos")]
        {
            Self {
                primary: "Option+Space".to_string(),
                fallback: Vec::new(),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            Self {
                primary: "Alt+Space".to_string(),
                fallback: Vec::new(),
            }
        }
    }
}

impl ShortcutPreferences {
    pub fn bindings(&self) -> Vec<String> {
        let mut seen: HashSet<String> = HashSet::new();
        let mut all: Vec<String> = Vec::new();

        let primary = self.primary.trim();
        if !primary.is_empty()
            && seen.insert(primary.to_ascii_lowercase()) {
                all.push(primary.to_string());
            }

        for fb in &self.fallback {
            let trimmed = fb.trim();
            if trimmed.is_empty() {
                continue;
            }
            if !seen.insert(trimmed.to_ascii_lowercase()) {
                continue;
            }
            all.push(trimmed.to_string());
        }

        all
    }

    pub fn update_primary(&mut self, primary: &str) -> Result<(), String> {
        let normalized = primary.trim();
        if normalized.is_empty() {
            return Err("shortcut cannot be empty".to_string());
        }
        Self::validate(normalized)?;
        Self::ensure_modifier(normalized)?;
        self.primary = normalized.to_string();
        self.normalize();
        Ok(())
    }

    pub fn reset_to_defaults(&mut self) {
        *self = Self::default();
    }

    pub fn normalize(&mut self) {
        self.ensure_platform_defaults();
        self.dedup();
    }

    pub fn validate(candidate: &str) -> Result<(), String> {
        Shortcut::from_str(candidate)
            .map(|_| ())
            .map_err(|e| format!("invalid shortcut: {e}"))
    }

    fn ensure_modifier(candidate: &str) -> Result<(), String> {
        let parts: Vec<&str> = candidate.split('+').collect();
        if parts.len() < 2 {
            return Err("shortcut must include at least one modifier key".to_string());
        }
        Ok(())
    }

    fn dedup(&mut self) {
        self.fallback
            .retain(|fb| !fb.eq_ignore_ascii_case(&self.primary));
        let mut seen = HashSet::new();
        self.fallback
            .retain(|fb| seen.insert(fb.trim().to_ascii_lowercase()));
    }

    fn ensure_platform_defaults(&mut self) {
        // No platform-specific fallback needed.
    }
}

impl Config {
    pub fn load() -> Self {
        match config_path() {
            Ok(path) => Self::load_from(&path),
            Err(_) => Self::default(),
        }
    }

    /// Load config from a specific path. A damaged file is moved aside
    /// before falling back to defaults, so the later default-save cannot
    /// destroy the user's original file.
    pub fn load_from(path: &Path) -> Self {
        match crate::persistence::read_json_or_quarantine(path) {
            Ok(contents) if !contents.is_empty() => {
                match serde_json::from_str::<Config>(&contents) {
                    Ok(parsed) => parsed.with_migrations(),
                    Err(_) => {
                        let _ = crate::persistence::quarantine(path);
                        Self::default()
                    }
                }
            }
            _ => Self::default(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = config_path()?;
        let payload = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        crate::persistence::write_atomically(&path, payload.as_bytes())
    }

    fn with_migrations(mut self) -> Self {
        if self.shortcuts.primary.trim().is_empty()
            || ShortcutPreferences::validate(&self.shortcuts.primary).is_err()
        {
            self.shortcuts = ShortcutPreferences::default();
        }
        self.shortcuts.ensure_platform_defaults();
        self.shortcuts.dedup();
        self
    }
}

fn config_path() -> Result<PathBuf, String> {
    Ok(data_root()?.join("config.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn temp_dir(label: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "limen-config-test-{}-{}-{}",
            std::process::id(),
            label,
            n
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    // 仕様: デフォルトのショートカットは必ずパース可能でなければならない
    fn default_primary_is_a_valid_shortcut() {
        let prefs = ShortcutPreferences::default();
        assert!(ShortcutPreferences::validate(&prefs.primary).is_ok());
        assert_eq!(prefs.bindings(), vec![prefs.primary.clone()]);
    }

    #[test]
    // 仕様: update_primary は正当な組み合わせを受け付け、primary を置き換える
    fn update_primary_accepts_valid_combo() {
        let mut prefs = ShortcutPreferences::default();
        prefs.update_primary("Command+Shift+P").expect("update");
        assert_eq!(prefs.primary, "Command+Shift+P");
    }

    #[test]
    // 仕様: 修飾キー無し・空文字・パース不能な組み合わせは拒否する
    fn update_primary_rejects_invalid_input() {
        let mut prefs = ShortcutPreferences::default();
        assert!(prefs.update_primary("").is_err());
        assert!(prefs.update_primary("K").is_err());
        assert!(prefs.update_primary("NotAKey+Q").is_err());
        assert_eq!(prefs.primary, ShortcutPreferences::default().primary);
    }

    #[test]
    // 仕様: reset_to_defaults でデフォルトに戻る
    fn reset_restores_defaults() {
        let mut prefs = ShortcutPreferences::default();
        prefs.update_primary("Command+Shift+P").expect("update");
        prefs.reset_to_defaults();
        assert_eq!(prefs.primary, ShortcutPreferences::default().primary);
    }

    #[test]
    // 仕様: 正常な設定ファイルはそのまま読み込む
    fn load_from_reads_valid_config() {
        let dir = temp_dir("valid");
        let path = dir.join("config.json");
        fs::write(
            &path,
            r#"{"shortcuts":{"primary":"Command+Shift+P","fallback":[]}}"#,
        )
        .unwrap();
        let cfg = Config::load_from(&path);
        assert_eq!(cfg.shortcuts.primary, "Command+Shift+P");
        assert!(path.exists());
    }

    #[test]
    // 仕様: 壊れた設定ファイルは退避してからデフォルトに戻す（上書きで消さない）
    fn load_from_quarantines_corrupt_config_before_defaulting() {
        let dir = temp_dir("corrupt");
        let path = dir.join("config.json");
        fs::write(&path, "{ not json").unwrap();

        let cfg = Config::load_from(&path);
        assert_eq!(cfg.shortcuts.primary, ShortcutPreferences::default().primary);
        assert!(!path.exists(), "corrupt file must be moved aside");

        let backups: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".corrupt-"))
            .collect();
        assert_eq!(backups.len(), 1);
    }

    #[test]
    // 仕様: onboarded のデフォルトは false
    fn onboarded_defaults_to_false() {
        assert!(!Config::default().onboarded);
    }

    #[test]
    // 仕様: onboarded フィールドを持たない旧設定は false として読める(後方互換)
    fn load_from_legacy_config_without_onboarded_is_false() {
        let dir = temp_dir("legacy");
        let path = dir.join("config.json");
        fs::write(&path, r#"{"shortcuts":{"primary":"Option+Space","fallback":[]}}"#).unwrap();
        let cfg = Config::load_from(&path);
        assert!(!cfg.onboarded);
    }

    #[test]
    // 仕様: onboarded=true を保存すると再読み込みでも維持される
    fn onboarded_flag_round_trips_true() {
        let dir = temp_dir("roundtrip");
        let path = dir.join("config.json");
        fs::write(
            &path,
            r#"{"shortcuts":{"primary":"Option+Space","fallback":[]},"onboarded":true}"#,
        )
        .unwrap();
        assert!(Config::load_from(&path).onboarded);
    }

    #[test]
    // 仕様: JSONとしては正しいが形が異なるファイルも退避してデフォルトに戻す
    fn load_from_quarantines_mismatched_shape() {
        let dir = temp_dir("shape");
        let path = dir.join("config.json");
        fs::write(&path, "[1, 2, 3]").unwrap();

        let cfg = Config::load_from(&path);
        assert_eq!(cfg.shortcuts.primary, ShortcutPreferences::default().primary);
        assert!(!path.exists(), "mismatched file must be moved aside");
    }

    #[test]
    // 仕様: ファイルが無い場合はデフォルトを返す（何も作らない）
    fn load_from_missing_file_returns_defaults() {
        let dir = temp_dir("missing");
        let path = dir.join("config.json");
        let cfg = Config::load_from(&path);
        assert_eq!(cfg.shortcuts.primary, ShortcutPreferences::default().primary);
        assert!(!path.exists());
    }
}
