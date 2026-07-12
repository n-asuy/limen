use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Write a file via tmp-file + rename so readers never observe partial
/// content, even if the process dies mid-write.
pub fn write_atomically(path: &Path, content: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("no parent directory for {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;

    let tmp = tmp_sibling(path);
    {
        let mut file = fs::File::create(&tmp).map_err(|e| e.to_string())?;
        file.write_all(content).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
    }
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        e.to_string()
    })
}

/// Read a persisted JSON file. A missing file yields an empty string.
/// A file with invalid JSON is moved aside (never deleted) and treated
/// as missing, so callers can safely re-initialize without destroying
/// the only remaining copy of user data.
pub fn read_json_or_quarantine(path: &Path) -> Result<String, String> {
    match fs::read_to_string(path) {
        Ok(contents) => {
            if serde_json::from_str::<serde_json::Value>(&contents).is_ok() {
                Ok(contents)
            } else {
                let backup = quarantine(path)?;
                log::error!(
                    "corrupt JSON at {}; moved aside to {}",
                    path.display(),
                    backup.display()
                );
                Ok(String::new())
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(format!("failed to read {}: {}", path.display(), e)),
    }
}

/// Move a damaged file aside with a timestamped suffix and return the
/// backup path.
pub fn quarantine(path: &Path) -> Result<PathBuf, String> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| format!("no file name in {}", path.display()))?;
    let backup = path.with_file_name(format!("{file_name}.corrupt-{ts}"));
    fs::rename(path, &backup).map_err(|e| e.to_string())?;
    Ok(backup)
}

fn tmp_sibling(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!("{file_name}.tmp{}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn temp_dir(label: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "limen-persistence-test-{}-{}-{}",
            std::process::id(),
            label,
            n
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    // 仕様: 新規ファイルを作成でき、一時ファイルは残らない
    fn write_atomically_creates_file_without_leftovers() {
        let dir = temp_dir("create");
        let path = dir.join("state.json");
        write_atomically(&path, b"{\"a\":1}").expect("write");
        assert_eq!(fs::read_to_string(&path).unwrap(), "{\"a\":1}");
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    // 仕様: 既存ファイルは新しい内容で置き換えられる
    fn write_atomically_replaces_existing_content() {
        let dir = temp_dir("replace");
        let path = dir.join("state.json");
        write_atomically(&path, b"old").expect("first write");
        write_atomically(&path, b"new").expect("second write");
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
    }

    #[test]
    // 仕様: 親ディレクトリが無くても作成して書き込む
    fn write_atomically_creates_missing_parent_dirs() {
        let dir = temp_dir("parents");
        let path = dir.join("nested").join("deeper").join("state.json");
        write_atomically(&path, b"x").expect("write");
        assert_eq!(fs::read_to_string(&path).unwrap(), "x");
    }

    #[test]
    // 仕様: ファイルが無い場合は空文字を返す（エラーにしない）
    fn read_missing_file_yields_empty_string() {
        let dir = temp_dir("missing");
        let path = dir.join("absent.json");
        assert_eq!(read_json_or_quarantine(&path).unwrap(), "");
    }

    #[test]
    // 仕様: 正常なJSONはそのまま返す
    fn read_valid_json_returns_content() {
        let dir = temp_dir("valid");
        let path = dir.join("state.json");
        fs::write(&path, "{\"spaces\":[]}").unwrap();
        assert_eq!(read_json_or_quarantine(&path).unwrap(), "{\"spaces\":[]}");
    }

    #[test]
    // 仕様: 壊れたJSONは退避して空文字を返す（元データは削除しない）
    fn read_corrupt_json_quarantines_and_yields_empty() {
        let dir = temp_dir("corrupt");
        let path = dir.join("state.json");
        fs::write(&path, "{ this is not json").unwrap();

        assert_eq!(read_json_or_quarantine(&path).unwrap(), "");
        assert!(!path.exists(), "corrupt original must be moved aside");

        let backups: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".corrupt-"))
            .collect();
        assert_eq!(backups.len(), 1);
        let backed_up = fs::read_to_string(backups[0].path()).unwrap();
        assert_eq!(backed_up, "{ this is not json");
    }

    #[test]
    // 仕様: quarantine は退避先パスを返し、内容を保全する
    fn quarantine_preserves_content_at_returned_path() {
        let dir = temp_dir("quarantine");
        let path = dir.join("config.json");
        fs::write(&path, "broken").unwrap();
        let backup = quarantine(&path).expect("quarantine");
        assert_eq!(fs::read_to_string(&backup).unwrap(), "broken");
        assert!(!path.exists());
    }
}
