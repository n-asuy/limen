//! Lightweight performance profiling for Limen.
//!
//! All public functions are gated behind `#[cfg(feature = "profiling")]`.
//! When the feature is disabled every call compiles to a no-op, so there is
//! zero overhead in release builds.
//!
//! Output: backend events as JSON Lines in
//! `<project_root>/log/performance/<date>_backend.jsonl`. Frontend events are
//! written separately by the dev server (see `src/perf/recorder.ts`).
//!
//! A dedicated background thread drains a bounded channel, batches entries,
//! and flushes them to disk — the recording call-site never blocks on I/O.

#[cfg(feature = "profiling")]
mod inner {
    use serde_json::{json, Value};
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::sync::OnceLock;
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

    static WRITER: OnceLock<mpsc::SyncSender<Value>> = OnceLock::new();

    // ---- date helpers (no chrono dependency) ----

    /// Civil date from days since Unix epoch (Howard Hinnant algorithm).
    fn civil_from_days(days: i64) -> (i64, u32, u32) {
        let z = days + 719_468;
        let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
        let doe = (z - era * 146_097) as u32;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
        let y = yoe as i64 + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = if m <= 2 { y + 1 } else { y };
        (y, m, d)
    }

    pub(crate) fn date_string() -> String {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let days = (secs / 86400) as i64;
        let (y, m, d) = civil_from_days(days);
        format!("{y:04}-{m:02}-{d:02}")
    }

    fn epoch_ms() -> f64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs_f64() * 1000.0)
            .unwrap_or(0.0)
    }

    // ---- log directory ----

    fn log_dir() -> PathBuf {
        // During `tauri dev`, cwd is src-tauri/. Walk up to the project root
        // by looking for Cargo.toml in the current dir (indicating we're
        // inside src-tauri/) and stepping one level up.
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let root = if cwd.join("Cargo.toml").exists() && cwd.join("tauri.conf.json").exists() {
            // We are inside src-tauri/ — go up to project root
            cwd.parent().map(|p| p.to_path_buf()).unwrap_or(cwd)
        } else {
            cwd
        };
        root.join("log").join("performance")
    }

    // ---- public API ----

    /// Milliseconds elapsed since `start`, rounded to 2 decimal places.
    pub fn elapsed_ms(start: Instant) -> f64 {
        let d = start.elapsed().as_secs_f64() * 1000.0;
        (d * 100.0).round() / 100.0
    }

    /// Initialise the background writer thread.  Call once at app startup.
    /// Subsequent calls are harmless no-ops.
    pub fn init() {
        // Bounded channel — if the writer cannot keep up we drop entries
        // rather than applying back-pressure to the hot path.
        const CHANNEL_CAP: usize = 4096;
        let (tx, rx) = mpsc::sync_channel::<Value>(CHANNEL_CAP);

        let dir = log_dir();
        std::thread::Builder::new()
            .name("perf-writer".into())
            .spawn(move || writer_loop(rx, dir))
            .expect("failed to spawn perf writer thread");

        let _ = WRITER.set(tx);
        log::info!("perf: profiling writer initialised");
    }

    fn writer_loop(rx: mpsc::Receiver<Value>, dir: PathBuf) {
        let _ = fs::create_dir_all(&dir);
        let date = date_string();
        let path = dir.join(format!("{date}_backend.jsonl"));

        let mut file = match fs::OpenOptions::new().create(true).append(true).open(&path) {
            Ok(f) => f,
            Err(e) => {
                log::error!("perf: failed to open {}: {e}", path.display());
                return;
            }
        };

        log::info!("perf: writing to {}", path.display());

        const BATCH_CAP: usize = 64;
        let mut buf: Vec<Value> = Vec::with_capacity(BATCH_CAP);

        // Block until an entry arrives; the loop ends when the channel closes.
        while let Ok(entry) = rx.recv() {
            buf.push(entry);
            // Drain any additional buffered entries
            while buf.len() < BATCH_CAP {
                match rx.try_recv() {
                    Ok(e) => buf.push(e),
                    Err(_) => break,
                }
            }
            for entry in buf.drain(..) {
                if let Ok(line) = serde_json::to_string(&entry) {
                    let _ = writeln!(file, "{line}");
                }
            }
            let _ = file.flush();
        }
    }

    /// Record a named performance event with arbitrary JSON fields.
    /// Non-blocking — the entry is sent to the background writer via channel.
    pub fn record(name: &str, fields: Value) {
        if let Some(tx) = WRITER.get() {
            let mut entry = json!({
                "ts": epoch_ms(),
                "name": name,
            });
            if let (Some(obj), Value::Object(f)) = (entry.as_object_mut(), fields) {
                for (k, v) in f {
                    obj.insert(k, v);
                }
            }
            // try_send: drop the entry silently if the channel is full
            let _ = tx.try_send(entry);
        }
    }
}

// ---- Feature-gated re-exports ----

#[cfg(feature = "profiling")]
pub use inner::{elapsed_ms, init, record};

// No-op stubs when the feature is disabled.
#[cfg(not(feature = "profiling"))]
#[inline(always)]
pub fn elapsed_ms(start: std::time::Instant) -> f64 {
    let d = start.elapsed().as_secs_f64() * 1000.0;
    (d * 100.0).round() / 100.0
}

#[cfg(not(feature = "profiling"))]
#[inline(always)]
pub fn init() {}

#[cfg(not(feature = "profiling"))]
#[inline(always)]
pub fn record(_name: &str, _fields: serde_json::Value) {}
