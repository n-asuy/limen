import type { SpaceAppsMap } from "./space-utils";

export type SpaceData = {
  id: number;
  name: string;
  emoji?: string;
  color?: string;
  visited: boolean;
  order: number;
  lastOpenedAt?: number | null;
};

export type PersistedState = {
  spaces: SpaceData[];
  spaceApps: SpaceAppsMap;
  appIcons: Record<string, string>;
};

export type LoadedState =
  | { kind: "missing" }
  | { kind: "invalid" }
  | { kind: "ok"; state: PersistedState };

const DEFAULT_EMOJIS = [
  "💻",
  "📚",
  "🧪",
  "🎨",
  "📧",
  "📝",
  "🧠",
  "📊",
  "🛠️",
  "🧩",
];

/**
 * Classify raw persisted text. "missing" means it is safe to write a fresh
 * default state; "invalid" means a file exists but cannot be trusted, so
 * callers must move it aside before writing anything.
 */
export function interpretPersistedText(txt: string): LoadedState {
  if (!txt || txt.trim() === "") return { kind: "missing" };
  try {
    const parsed = JSON.parse(txt);
    if (
      parsed &&
      Array.isArray(parsed.spaces) &&
      parsed.spaceApps !== null &&
      typeof parsed.spaceApps === "object" &&
      !Array.isArray(parsed.spaceApps) &&
      parsed.appIcons !== null &&
      typeof parsed.appIcons === "object" &&
      !Array.isArray(parsed.appIcons)
    ) {
      return { kind: "ok", state: parsed as PersistedState };
    }
    return { kind: "invalid" };
  } catch {
    return { kind: "invalid" };
  }
}

export function buildDefaultSpace(id: number): SpaceData {
  return {
    id,
    name: `Space ${id}`,
    emoji: DEFAULT_EMOJIS[(id - 1) % DEFAULT_EMOJIS.length],
    color: undefined,
    visited: false,
    order: id - 1,
    lastOpenedAt: null,
  };
}

export function buildDefaultSpaces(count: number): SpaceData[] {
  return Array.from({ length: count }).map((_, i) => buildDefaultSpace(i + 1));
}

/** Pad a saved space list up to `count` entries, keeping existing data. */
export function ensureSpaceCount(
  spaces: SpaceData[],
  count: number,
): { spaces: SpaceData[]; changed: boolean } {
  if (spaces.length >= count) return { spaces, changed: false };
  const additions = Array.from({ length: count - spaces.length }).map((_, i) =>
    buildDefaultSpace(spaces.length + i + 1),
  );
  return { spaces: [...spaces, ...additions], changed: true };
}

/**
 * Icons are stored as file paths. Older versions stored base64 data URLs;
 * drop the whole map when any legacy value is present.
 */
export function cleanIconPaths(icons: Record<string, string>): {
  icons: Record<string, string>;
  changed: boolean;
} {
  const bad = Object.values(icons).some(
    (v) => v && !(v.endsWith(".png") || v.startsWith("/") || v.includes(":\\")),
  );
  return bad ? { icons: {}, changed: true } : { icons, changed: false };
}
