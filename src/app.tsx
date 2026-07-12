import { useEffect, useState, useCallback, useRef, useMemo, type CSSProperties } from "react";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { useSpaceStore, type SpaceData } from "./hooks/use-space-store";
import {
  markWindowShown,
  markFirstHover,
  trackListenerSubscribe,
  startPerfTimer,
  startLongTaskTracking,
  startPerfRecording,
} from "./perf/recorder";
import "./app.css";

const isSameLocalDay = (a: Date, b: Date) =>
  a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();

const formatTodayVisitMessage = (
  timestamp: number | null | undefined,
  now: Date,
  formatter: Intl.DateTimeFormat,
) => {
  if (!timestamp) return "Not opened yet today";
  const visitDate = new Date(timestamp);
  if (!isSameLocalDay(visitDate, now)) return "Not opened yet today";
  return `Opened today at ${formatter.format(visitDate)}`;
};

function App() {
  const [activeIndex, setActiveIndex] = useState(0);
  const [currentActiveId, setCurrentActiveId] = useState<number | null>(null);
  const [recentTrail, setRecentTrail] = useState<number[]>([]);
  const { spaces, markAsVisited, updateSpace, recordApps, topApps, getIcon } = useSpaceStore();
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editValue, setEditValue] = useState<string>("");
  const editInputRef = useRef<HTMLInputElement | null>(null);
  const [isOptionDown, setIsOptionDown] = useState(false);
  const timeFormatter = useMemo(
    () =>
      new Intl.DateTimeFormat(undefined, {
        hour: "2-digit",
        minute: "2-digit",
      }),
    [],
  );
  const updateSpaceRef = useRef(updateSpace);

  useEffect(() => {
    updateSpaceRef.current = updateSpace;
  }, [updateSpace]);

  useEffect(() => {
    if (currentActiveId === null) return;
    const timestamp = Date.now();
    updateSpaceRef.current(currentActiveId, { lastOpenedAt: timestamp }).catch(() => undefined);
  }, [currentActiveId]);

  const lastActiveIdRef = useRef<number | null>(null);

  useEffect(() => {
    // Listen for macOS active Space changes
    const unlistenPromise = listen("space-changed", () => {
      console.debug("[Limen] space-changed received");
      invoke<number | null>("get_active_space_index")
        .then((v) => {
          const nextId = v ?? null;
          if (lastActiveIdRef.current === nextId) return;
          lastActiveIdRef.current = nextId;
          setCurrentActiveId(nextId);
          console.debug("[Limen] active space:", nextId);
          // Backend no polling now; fetch on demand if needed
        })
        .catch(() => {});
    });
    return () => {
      unlistenPromise.then((un) => un());
    };
  }, []);

  // 初期アクティブSpace読み込み
  useEffect(() => {
    invoke<number | null>("get_active_space_index")
      .then((v) => {
        const id = v ?? null;
        lastActiveIdRef.current = id;
        setCurrentActiveId(id);
      })
      .catch(() => setCurrentActiveId(null));
  }, []);

  useEffect(() => {
    if (currentActiveId === null) return;
    setRecentTrail((prev) => {
      const deduped = prev.filter((id) => id !== currentActiveId);
      return [currentActiveId, ...deduped].slice(0, 3);
    });
  }, [currentActiveId]);

  const visibleSpaces = spaces.slice(0, 9);
  const now = new Date();

  useEffect(() => {
    const resize = async () => {
      try {
        const win = getCurrentWindow();
        await win.setSize(new LogicalSize(496, 496));
        await win.setAlwaysOnTop(true);
      } catch {
        /* ignored outside Tauri runtime */
      }
    };

    resize();
    startLongTaskTracking();
    startPerfRecording();
  }, []);

  // Track window show→hover latency via backend event
  useEffect(() => {
    const unlistenPromise = listen("perf:window-shown", () => {
      markWindowShown();
    });
    return () => {
      unlistenPromise.then((un) => un());
    };
  }, []);

  const handleSwitchError = useCallback(
    (error: unknown) => {
      const message = error instanceof Error ? error.message : String(error ?? "");
      if (message.includes("Accessibility permission not granted")) {
        console.warn("[Limen] Switch failed: accessibility not granted");
        // Backend shows the system prompt on first call and opens the
        // System Settings Accessibility pane on later ones.
        invoke("request_accessibility").catch(() => undefined);
        return;
      }
      if (message.includes("out of range")) {
        console.warn("[Limen] Switch failed (space index out of supported range)");
        return;
      }
      console.warn("[Limen] Switch failed", message);
    },
    [],
  );

  // Listen backend events: apps-visible and frontmost-changed
  // Refs are used inside callbacks to avoid re-subscribing on every state change.
  const recordAppsRef = useRef(recordApps);
  useEffect(() => { recordAppsRef.current = recordApps; }, [recordApps]);

  useEffect(() => {
    trackListenerSubscribe();
    const un1 = listen<{ space_id?: number | null; apps?: { bundle_id: string; name: string }[] }>(
      "apps-visible",
      (e) => {
        const endTimer = startPerfTimer("event_apps_visible");
        const p = e.payload as any;
        const sid = (p?.space_id ?? null) as number | null;
        const targetId = sid ?? lastActiveIdRef.current;
        if (!targetId) { endTimer({ skipped: true }); return; }
        const mapped = (p?.apps ?? []).map((a: any) => ({ bundleId: a.bundle_id, name: a.name }));
        recordAppsRef.current(targetId, mapped);
        endTimer({ space_id: targetId, app_count: mapped.length });
      },
    );
    const un2 = listen<{ space_id?: number | null; bundle_id: string; name: string }>(
      "frontmost-changed",
      (e) => {
        const endTimer = startPerfTimer("event_frontmost_changed");
        const p = e.payload as any;
        const sid = (p?.space_id ?? null) as number | null;
        const targetId = sid ?? lastActiveIdRef.current;
        if (!targetId) { endTimer({ skipped: true }); return; }
        recordAppsRef.current(targetId, [{ bundleId: p.bundle_id, name: p.name }]);
        endTimer({ space_id: targetId });
      },
    );
    return () => {
      un1.then((f) => f());
      un2.then((f) => f());
    };
  }, []);

  const onKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.altKey || e.key === "Alt") {
        setIsOptionDown(true);
      }

      // When editing a label, intercept keys and avoid switching behavior
      if (editingId !== null) {
        if (e.key === "Enter") {
          e.preventDefault();
          const name = editValue.trim();
          if (name) {
            updateSpace(editingId, { name });
          }
          setEditingId(null);
          return;
        }
        if (e.key === "Escape") {
          e.preventDefault();
          setEditingId(null);
          return;
        }
        // Ignore other keys at overlay level while editing
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        invoke("hide_window");
        return;
      }
      // Row navigation: 左右/上下で循環
      if (e.key === "ArrowRight" || e.key === "ArrowDown")
        setActiveIndex((i) => (i + 1) % Math.max(visibleSpaces.length, 1));
      if (e.key === "ArrowLeft" || e.key === "ArrowUp")
        setActiveIndex((i) => (i - 1 + Math.max(visibleSpaces.length, 1)) % Math.max(visibleSpaces.length, 1));

      if (e.key === "Enter") {
        e.preventDefault();
        const targetSpace = visibleSpaces[activeIndex];
        if (targetSpace) {
          markAsVisited(targetSpace.id);
          invoke("switch_space", { index: targetSpace.id })
            .then(() => {
              setCurrentActiveId(targetSpace.id);
              invoke("hide_window");
            })
            .catch((err) => {
              // keep overlay open; handler prompts for permissions when required
              handleSwitchError(err);
            });
        }
      }
      // Number selection 1..9 only
      const directDigit = /^[1-9]$/.test(e.key) ? parseInt(e.key, 10) : null;
      const codeDigitMatch = /^Digit([1-9])$/.exec(e.code ?? "");
      const parsedDigit = directDigit ?? (codeDigitMatch ? parseInt(codeDigitMatch[1], 10) : null);
      if (parsedDigit !== null) {
        e.preventDefault();
        const idx = parsedDigit - 1;
        if (idx < visibleSpaces.length) {
          setActiveIndex(idx);
          const targetSpace = visibleSpaces[idx];
          if (targetSpace) {
            markAsVisited(targetSpace.id);
            invoke("switch_space", { index: targetSpace.id })
              .then(() => {
                setCurrentActiveId(targetSpace.id);
                invoke("hide_window");
              })
              .catch(handleSwitchError);
          }
        }
      }
    },
    [
      activeIndex,
      visibleSpaces,
      markAsVisited,
      editingId,
      editValue,
      updateSpace,
      handleSwitchError,
    ],
  );

  const onKeyUp = useCallback((e: KeyboardEvent) => {
    if (!e.altKey) {
      setIsOptionDown(false);
    }
  }, []);

  useEffect(() => {
    const handleBlur = () => setIsOptionDown(false);
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);
    window.addEventListener("blur", handleBlur);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
      window.removeEventListener("blur", handleBlur);
    };
  }, [onKeyDown, onKeyUp]);

  // Focus the input when entering edit mode
  useEffect(() => {
    if (editingId !== null) {
      // next tick focus
      setTimeout(() => {
        editInputRef.current?.focus();
        editInputRef.current?.select();
      }, 0);
    }
  }, [editingId]);

  const startEditing = (id: number, currentName: string) => {
    setEditingId(id);
    setEditValue(currentName);
  };

  const commitEdit = () => {
    if (editingId === null) return;
    const name = editValue.trim();
    if (name) {
      updateSpace(editingId, { name });
    }
    setEditingId(null);
  };

  // --- Ring layout helpers ---
  const ringItemStyle = (angleDeg: number, radiusCss: string): CSSProperties => ({
    position: "absolute",
    top: "50%",
    left: "50%",
    transform: `translate(-50%, -50%) rotate(${angleDeg}deg) translate(${radiusCss}) rotate(${-angleDeg}deg)`,
  });
  const OUTER_RADIUS_CSS = "var(--label-r)";

  const lastVisitedId = recentTrail[1] ?? null;
  const secondVisitedId = recentTrail[2] ?? null;

  const fileUrl = (path?: string): string | undefined => (path ? convertFileSrc(path) : undefined);

  // Allow dragging the window by grabbing the ring band
  const onDragMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button !== 0) return; // left click only
    getCurrentWindow().startDragging().catch(() => {});
  }, []);

  const selectSpaceById = useCallback((spaceId: number) => {
    const s = spaces.find((x) => x.id === spaceId);
    if (!s) return;
    markAsVisited(s.id);
    invoke("switch_space", { index: s.id })
      .then(() => {
        setCurrentActiveId(s.id);
        invoke("hide_window");
      })
      .catch(handleSwitchError);
  }, [spaces, markAsVisited, handleSwitchError]);

  return (
    <div className="screen">
      <div className="ring">
        <div className="ring-bg" data-tauri-drag-region onMouseDown={onDragMouseDown} />
        {/* Inner rim bevel for donut inner edge (neumorphism) */}
        <div className="ring-inner-bevel" aria-hidden />
        {/* Center transparent drag handle inside the ring */}
        <div
          className="ring-drag"
          data-tauri-drag-region
          onMouseDown={onDragMouseDown}
          aria-label="Drag window"
        />
        {/* Pie-sector highlights using rotate+skew (reference: react-pie-menu) */}
        <ul className="pie-list" aria-hidden>
          {/* Wide invisible hit zones per sector to improve hover/click responsiveness */}
          {visibleSpaces.map((s: SpaceData, j: number) => {
            const step = 360 / visibleSpaces.length;
            const delta = 90 - step;
            const start = delta + step / 2;
            const liStyle: CSSProperties = {
              transform: `rotate(${start + j * step}deg) skew(${delta}deg)`,
            };
            const innerStyle: CSSProperties = {
              transform: `skew(${-delta}deg) rotate(${(step / 2) - 90}deg)`,
            };
            return (
              <li key={`hit-${s.id}`} className="pie-item" style={liStyle}
                  onMouseEnter={() => { markFirstHover(); setActiveIndex(j); }}
                  onClick={() => selectSpaceById(s.id)}>
                <div className="pie-hit" style={innerStyle} />
              </li>
            );
          })}
          {currentActiveId !== null && visibleSpaces.length > 0 ? (() => {
            const step = 360 / visibleSpaces.length;
            const delta = 90 - step;
            const start = delta + step / 2; // align to top
            const idx = visibleSpaces.findIndex((s) => s.id === currentActiveId);
            if (idx >= 0) {
              const liStyle: CSSProperties = {
                transform: `rotate(${start + idx * step}deg) skew(${delta}deg)`,
              };
              const innerStyle: CSSProperties = {
                transform: `skew(${-delta}deg) rotate(${(step / 2) - 90}deg)`,
              };
              return (
                <li className="pie-item" style={liStyle}>
                  <div className="pie-slice current" style={innerStyle} />
                </li>
              );
            }
            return null;
          })() : null}
          {visibleSpaces.length > 0 ? (() => {
            const step = 360 / visibleSpaces.length;
            const delta = 90 - step;
            const start = delta + step / 2;
            const liStyle: CSSProperties = {
              transform: `rotate(${start + activeIndex * step}deg) skew(${delta}deg)`,
            };
            const innerStyle: CSSProperties = {
              transform: `skew(${-delta}deg) rotate(${(step / 2) - 90}deg)`,
            };
            return (
              <li className="pie-item" style={liStyle}>
                <div className="pie-slice selection" style={innerStyle} />
              </li>
            );
          })() : null}
        </ul>
        {visibleSpaces.map((s: SpaceData, j: number) => {
          const step = visibleSpaces.length > 0 ? 360 / visibleSpaces.length : 0;
          const angle = j * step - 90; // start at top
          const commonClass =
            "ring-item" + (j === activeIndex ? " active" : "") + (currentActiveId === s.id ? " current" : "");
          const indicatorState =
            s.id === lastVisitedId
              ? {
                  className: "recent-indicator recent-indicator--last",
                  tooltip: "Previously active workspace",
                }
              : s.id === secondVisitedId
                ? {
                    className: "recent-indicator recent-indicator--second",
                    tooltip: "Visited two switches ago",
                  }
                : null;
          const todayTooltip = formatTodayVisitMessage(s.lastOpenedAt ?? null, now, timeFormatter);
          const tooltipLines = [todayTooltip, indicatorState?.tooltip].filter(Boolean);
          const combinedTooltip =
            tooltipLines.length > 0 ? tooltipLines.join("\n") : undefined;
          const commonProps = {
            className: commonClass,
            style: ringItemStyle(angle, OUTER_RADIUS_CSS),
            onMouseEnter: () => setActiveIndex(j),
            title: combinedTooltip,
          } as const;

          const displayNumber = j + 1;
          const content = (
            <>
              <div className="ring-icon-slot">
                {indicatorState ? (
                  <span className={indicatorState.className} aria-hidden="true" />
                ) : null}
                {isOptionDown && displayNumber <= 9 ? (
                  <div className="shortcut-hint" aria-hidden="true">
                    <span className="shortcut-hint-symbol">⌥</span>
                    <span className="shortcut-hint-number">{displayNumber}</span>
                  </div>
                ) : null}
                {(() => {
                  const top3 = topApps(s.id, 3);
                  const icons = top3
                    .map((a) => ({ id: a.bundleId, src: fileUrl(getIcon(a.bundleId)), name: a.name }))
                    .filter((x): x is { id: string; src: string; name: string } => !!x.src);
                  if (icons.length > 0) {
                    return (
                      <div className="app-icons app-icons--stack">
                        {icons.map((ic, i) => (
                          <img
                            key={`${s.id}-${ic.id}`}
                            className={`app-icon app-icon--lg app-icon--p${i}`}
                            src={ic.src}
                            alt={ic.name}
                            draggable={false}
                          />
                        ))}
                      </div>
                    );
                  }
                  // No icon yet: keep vertical space with an empty placeholder
                  return <div className="app-icons app-icons--placeholder" aria-hidden />;
                })()}
              </div>
              {editingId === s.id ? (
                <input
                  ref={editInputRef}
                  className="label-input"
                  value={editValue}
                  onChange={(e) => setEditValue(e.target.value)}
                  onClick={(e) => e.stopPropagation()}
                  onMouseDown={(e) => e.stopPropagation()}
                  onKeyDown={(e) => {
                    e.stopPropagation();
                    if (e.key === "Enter") {
                      e.preventDefault();
                      commitEdit();
                    }
                  }}
                  onBlur={commitEdit}
                />
              ) : (
                <div
                  className="label"
                  onClick={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    startEditing(s.id, s.name);
                  }}
                  onMouseDown={(e) => e.stopPropagation()}
                >
                  {s.name}
                </div>
              )}
            </>
          );

          // Avoid <button> semantics while editing to prevent Space from triggering click
          if (editingId === s.id) {
            return (
              <div key={`ring-${s.id}`} {...commonProps}>
                {content}
              </div>
            );
          }

          return (
            <button
              key={`ring-${s.id}`}
              {...commonProps}
              onClick={() => selectSpaceById(s.id)}
            >
              {content}
            </button>
          );
        })}
      </div>
    </div>
  );
}

export default App;
