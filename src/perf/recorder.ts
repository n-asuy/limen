/**
 * Lightweight frontend performance recorder for Limen.
 *
 * Buffers structured events and periodically flushes them to the Vite
 * dev server via `POST /perf/report`, which writes them to
 * `<project_root>/log/performance/<date>_frontend.jsonl`.
 *
 * Recording is opt-in via `VITE_PROFILING=1`, the counterpart of the
 * backend's `profiling` cargo feature; `bun run dev:perf` sets both. When it
 * is off every entry point below is a no-op, and because the flag is inlined
 * at build time the whole recorder is dropped from production bundles.
 *
 * Usage:
 *   const end = startPerfTimer("some_operation", { extra: "field" });
 *   // ... work ...
 *   end({ result_count: 42 });
 *
 *   recordPerfEvent("one_shot_event", { value: 123 });
 */

const PROFILING = import.meta.env.VITE_PROFILING === "1";

// ---- types ----

export type PerfFields = Record<string, string | number | boolean | null>;
export type EndTimerFn = (extra?: PerfFields) => void;

const NOOP_TIMER: EndTimerFn = () => {};

interface PerfEntry {
  ts: string; // ISO 8601
  type: string;
  name: string;
  [key: string]: unknown;
}

// ---- buffer & flush ----

const MAX_BUFFER = 5_000;
const FLUSH_INTERVAL_MS = 30_000;

const buffer: PerfEntry[] = [];
let flushTimer: ReturnType<typeof setInterval> | null = null;
let flushing = false;

function ensureFlushTimer() {
  if (flushTimer !== null) return;
  flushTimer = setInterval(() => void flush(), FLUSH_INTERVAL_MS);
}

async function sendEntries(entries: PerfEntry[], keepalive?: boolean): Promise<void> {
  const res = await fetch("/perf/report", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(entries),
    keepalive,
  });
  if (!res.ok) throw new Error(`perf report failed: ${res.status}`);
}

async function flush(): Promise<void> {
  if (flushing || buffer.length === 0) return;
  flushing = true;
  const entries = buffer.splice(0, buffer.length);
  try {
    await sendEntries(entries, true);
  } catch {
    // Re-queue if there is room
    if (buffer.length + entries.length <= MAX_BUFFER) {
      buffer.unshift(...entries);
    }
  } finally {
    flushing = false;
  }
}

function flushOnPageExit(): void {
  if (flushing || buffer.length === 0) return;
  const entries = buffer.splice(0);
  const payload = JSON.stringify(entries);
  const body = new Blob([payload], { type: "application/json" });
  const sent = navigator.sendBeacon("/perf/report", body);
  if (!sent) {
    if (buffer.length + entries.length <= MAX_BUFFER) {
      buffer.unshift(...entries);
    }
    void flush();
  }
}

// ---- core API ----

function now(): string {
  return new Date().toISOString();
}

function roundMs(v: number): number {
  return Math.round(v * 100) / 100;
}

function push(entry: PerfEntry): void {
  if (!PROFILING) return;
  if (buffer.length < MAX_BUFFER) {
    buffer.push(entry);
  }
  ensureFlushTimer();
}

/**
 * Record a one-shot performance event.
 */
export function recordPerfEvent(name: string, fields?: PerfFields): void {
  push({ ts: now(), type: "ui-action", name, ...fields });
}

/**
 * Start a timer.  Returns a callback that, when called, records the event
 * with `duration_ms` automatically calculated.
 */
export function startPerfTimer(name: string, fields?: PerfFields): EndTimerFn {
  if (!PROFILING) return NOOP_TIMER;
  const t0 = performance.now();
  return (extra?: PerfFields) => {
    const duration_ms = roundMs(performance.now() - t0);
    push({ ts: now(), type: "ui-action", name, duration_ms, ...fields, ...extra });
  };
}

// ---- specialised trackers ----

/**
 * Track the time from window-show to the first mouse-move/hover on the ring.
 * Call `markWindowShown()` when the overlay becomes visible, and
 * `markFirstHover()` when the first mouseenter fires on a ring item.
 */
let windowShownAt: number | null = null;
let hoverRecorded = false;

export function markWindowShown(): void {
  windowShownAt = performance.now();
  hoverRecorded = false;
}

export function markFirstHover(): void {
  if (hoverRecorded || windowShownAt === null) return;
  hoverRecorded = true;
  const latency_ms = roundMs(performance.now() - windowShownAt);
  recordPerfEvent("window_show_to_first_hover", { latency_ms });
  windowShownAt = null;
}

/**
 * Count listener re-subscriptions to detect churn.
 */
let listenerSubscribeCount = 0;

export function trackListenerSubscribe(): void {
  listenerSubscribeCount += 1;
  recordPerfEvent("listener_subscribe", { total: listenerSubscribeCount });
}

// ---- Web Vitals (long tasks) ----

let longTaskObserver: PerformanceObserver | null = null;

export function startLongTaskTracking(): void {
  if (!PROFILING) return;
  if (longTaskObserver) return;
  if (typeof PerformanceObserver === "undefined") return;
  try {
    longTaskObserver = new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) {
        push({
          ts: now(),
          type: "long-task",
          name: "long_task",
          duration_ms: roundMs(entry.duration),
          start_ms: roundMs(entry.startTime),
        });
      }
    });
    longTaskObserver.observe({ type: "longtask", buffered: true });
  } catch {
    // longtask observer not supported in this WebView
  }
}

// ---- lifecycle ----

export function startPerfRecording(): void {
  if (!PROFILING) return;
  const onVisibilityChange = () => {
    if (document.visibilityState === "hidden") flushOnPageExit();
  };
  document.addEventListener("visibilitychange", onVisibilityChange);
  window.addEventListener("beforeunload", () => flushOnPageExit());
}

export function stopPerfRecording(): void {
  if (flushTimer !== null) {
    clearInterval(flushTimer);
    flushTimer = null;
  }
  void flush();
  longTaskObserver?.disconnect();
  longTaskObserver = null;
}
