import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { topAppsPure } from "../lib/space-utils";
import {
  buildDefaultSpaces,
  cleanIconPaths,
  ensureSpaceCount,
  interpretPersistedText,
  type PersistedState,
  type SpaceData,
} from "../lib/persisted-state";
import { startPerfTimer } from "../perf/recorder";

export type { SpaceData } from "../lib/persisted-state";

export type SpaceApp = {
  bundleId: string;
  name: string;
  count: number;
  lastSeen: number; // epoch ms
};

type SpaceAppsMap = Record<string, Record<string, SpaceApp>>; // spaceId -> bundleId -> data

const MAX_APPS_PER_SPACE = 3;

export function useSpaceStore() {
  const [spaces, setSpaces] = useState<SpaceData[]>([]);
  const [spaceApps, setSpaceApps] = useState<SpaceAppsMap>({});
  const [appIcons, setAppIcons] = useState<Record<string, string>>({});
  const [iconsDir, setIconsDir] = useState<string | null>(null);
  const spacesRef = useRef<SpaceData[]>([]);
  const spaceAppsRef = useRef<SpaceAppsMap>({});
  const appIconsRef = useRef<Record<string, string>>({});
  const iconsDirRef = useRef<string | null>(null);

  const DEFAULT_COUNT = 9;

  // Persist all state to ~/Library/Application Support/Limen/space.json
  const persistState = useCallback(async (state: PersistedState) => {
    const payload = JSON.stringify(state);
    const endTimer = startPerfTimer("persist_state", { bytes: payload.length });
    try {
      await invoke("save_state_file", { content: payload });
    } catch (e) {
      console.error("Failed to save state via tauri command:", e);
    } finally {
      endTimer();
    }
  }, []);

  useEffect(() => {
    const applyState = (
      nextSpaces: SpaceData[],
      nextApps: SpaceAppsMap,
      nextIcons: Record<string, string>,
    ) => {
      spacesRef.current = nextSpaces;
      spaceAppsRef.current = nextApps;
      appIconsRef.current = nextIcons;
      setSpaces(nextSpaces);
      setSpaceApps(nextApps);
      setAppIcons(nextIcons);
    };

    const initStore = async () => {
      // resolve icons directory path (for file-based icons)
      try {
        const dir = await invoke<string>("get_icons_dir");
        setIconsDir(dir);
      } catch {}

      // Load ~/Library/Application Support/Limen/space.json.
      // A read failure is NOT the same as a missing file: writing defaults
      // over a file we merely failed to read would destroy user data.
      let loaded: ReturnType<typeof interpretPersistedText> | { kind: "error" };
      try {
        const txt = (await invoke<string>("load_state_file")) || "";
        loaded = interpretPersistedText(txt);
      } catch (e) {
        console.error("Failed to read persisted state:", e);
        loaded = { kind: "error" };
      }

      if (loaded.kind === "ok") {
        const ensured = ensureSpaceCount(loaded.state.spaces, DEFAULT_COUNT);
        const cleaned = cleanIconPaths(loaded.state.appIcons);
        applyState(ensured.spaces, loaded.state.spaceApps, cleaned.icons);
        if (ensured.changed || cleaned.changed) {
          await persistState({
            spaces: ensured.spaces,
            spaceApps: loaded.state.spaceApps,
            appIcons: cleaned.icons,
          });
        }
        return;
      }

      // Defaults from here on. Persist them only when the previous file is
      // genuinely absent, or has been safely moved aside.
      let safeToPersist = loaded.kind === "missing";
      if (loaded.kind === "invalid") {
        try {
          const backup = await invoke<string | null>("quarantine_state_file");
          if (backup) console.warn("Unreadable state moved aside to:", backup);
          safeToPersist = true;
        } catch (e) {
          console.error("Failed to quarantine unreadable state:", e);
        }
      }

      const defaults = buildDefaultSpaces(DEFAULT_COUNT);
      applyState(defaults, {}, {});
      if (safeToPersist) {
        await persistState({ spaces: defaults, spaceApps: {}, appIcons: {} });
      }
    };

    initStore();
  }, []);

  useEffect(() => {
    spacesRef.current = spaces;
  }, [spaces]);

  useEffect(() => {
    spaceAppsRef.current = spaceApps;
  }, [spaceApps]);

  useEffect(() => {
    appIconsRef.current = appIcons;
  }, [appIcons]);

  useEffect(() => {
    iconsDirRef.current = iconsDir;
  }, [iconsDir]);

  const updateSpace = async (id: number, updates: Partial<SpaceData>) => {
    const updatedSpaces = spacesRef.current.map((space) =>
      space.id === id ? { ...space, ...updates } : space,
    );
    spacesRef.current = updatedSpaces;
    setSpaces(updatedSpaces);
    await persistState({ spaces: updatedSpaces, spaceApps: spaceAppsRef.current, appIcons: appIconsRef.current });
  };

  const markAsVisited = async (id: number) => {
    await updateSpace(id, { visited: true });
  };

  const recordApps = useCallback(async (
    spaceId: number,
    apps: { bundleId: string; name: string; iconBase64?: string | null }[],
  ) => {
    const endTimer = startPerfTimer("record_apps", { space_id: spaceId, incoming: apps.length });
    const now = Date.now();
    const key = String(spaceId);
    const prevSpaceApps = spaceAppsRef.current;
    const bucket = { ...(prevSpaceApps[key] || {}) };

    for (const a of apps) {
      const prevEntry = bucket[a.bundleId];
      bucket[a.bundleId] = {
        bundleId: a.bundleId,
        name: a.name,
        count: (prevEntry?.count ?? 0) + 1,
        lastSeen: now,
      };
    }

    const trimmedEntries = Object.values(bucket)
      .sort((a, b) => b.lastSeen - a.lastSeen)
      .slice(0, MAX_APPS_PER_SPACE);
    const prunedBucket = trimmedEntries.reduce<Record<string, SpaceApp>>((acc, app) => {
      acc[app.bundleId] = app;
      return acc;
    }, {});

    const nextSpaceApps = { ...prevSpaceApps, [key]: prunedBucket } as SpaceAppsMap;
    spaceAppsRef.current = nextSpaceApps;
    setSpaceApps(nextSpaceApps);

    let nextAppIcons = appIconsRef.current;
    const currentIconsDir = iconsDirRef.current;
    if (currentIconsDir) {
      const updated = { ...nextAppIcons };
      let changed = false;
      for (const a of apps) {
        const sanitized = a.bundleId.replaceAll("/", "_");
        const path = `${currentIconsDir}/${sanitized}.png`;
        if (updated[a.bundleId] !== path) {
          updated[a.bundleId] = path;
          changed = true;
        }
      }
      if (changed) {
        nextAppIcons = updated;
        appIconsRef.current = updated;
        setAppIcons(updated);
      }
    }

    await persistState({ spaces: spacesRef.current, spaceApps: nextSpaceApps, appIcons: nextAppIcons });
    const totalBundles = Object.values(nextSpaceApps).reduce(
      (acc, appMap) => acc + Object.keys(appMap).length,
      0,
    );
    endTimer({
      bucket_size: Object.keys(prunedBucket).length,
      total_bundles: totalBundles,
      icons_count: Object.keys(nextAppIcons).length,
    });
  }, [persistState]);

  const topApps = (spaceId: number, n = 3): SpaceApp[] =>
    topAppsPure(spaceApps as any, spaceId, n) as any;

  const getIcon = (bundleId: string): string | undefined => {
    // Prefer saved file path if it looks like a path; otherwise derive
    const mapped = appIcons[bundleId];
    if (mapped) {
      const looksLikePath =
        mapped.endsWith(".png") ||
        mapped.startsWith("/") ||
        mapped.includes(":\\");
      if (looksLikePath) return mapped;
    }
    if (!iconsDir) return undefined;
    const sanitized = bundleId.replaceAll("/", "_");
    return `${iconsDir}/${sanitized}.png`;
  };

  return {
    spaces: [...spaces].sort((a, b) => a.order - b.order),
    updateSpace,
    markAsVisited,
    recordApps,
    topApps,
    getIcon,
  };
}
