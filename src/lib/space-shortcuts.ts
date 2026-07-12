/** Spaces reachable from the ring, and therefore from Ctrl+1-9. */
export const SPACE_SHORTCUT_COUNT = 9;

export type SpaceShortcutStatus = "ready" | "partial" | "disabled";

export type SpaceShortcutSummary = {
  status: SpaceShortcutStatus;
  /** Spaces whose Mission Control shortcut is enabled and bound to Ctrl+N. */
  enabled: number[];
  /** Spaces that will not switch until their shortcut is enabled. */
  missing: number[];
};

export function summarizeSpaceShortcuts(available: readonly number[]): SpaceShortcutSummary {
  const all = Array.from({ length: SPACE_SHORTCUT_COUNT }, (_, i) => i + 1);
  const reported = new Set(available);
  const enabled = all.filter((index) => reported.has(index));
  const missing = all.filter((index) => !reported.has(index));
  const status = missing.length === 0 ? "ready" : enabled.length === 0 ? "disabled" : "partial";
  return { status, enabled, missing };
}

/** Spelled out in full even though the "Enable..." button opens the Keyboard
 *  pane: that button rides an unofficial URL scheme that a future macOS can
 *  break silently, and the written path is what survives it. */
const SETUP_STEPS =
  'Enable "Switch to Desktop 1..9" in System Settings → Keyboard → Keyboard Shortcuts → Mission Control.';

/** Ties the row to what the reader actually does (pick a Space in the ring),
 *  not to the key events Limen injects. Ctrl+1-9 stays in the copy because
 *  that is how the rows are labelled in System Settings. */
const RING_TO_MACOS = "Picking a Space in the ring triggers macOS's own shortcut for that Desktop (Ctrl+1–9).";

/** The badge carries the status; the copy carries the meaning, and the setup
 *  steps only when there is something to do. */
export function describeSpaceShortcuts(summary: SpaceShortcutSummary | null): string {
  if (summary?.status === "ready") {
    return RING_TO_MACOS;
  }
  if (summary?.status === "partial") {
    return `${RING_TO_MACOS} Spaces ${formatSpaceRanges(summary.missing)} have no working shortcut, so picking them does nothing. ${SETUP_STEPS}`;
  }
  return `${RING_TO_MACOS} macOS keeps those shortcuts disabled by default, so no Space will switch until you turn them on. ${SETUP_STEPS}`;
}

/** "5–9", "2, 5–6", "4", "" */
export function formatSpaceRanges(indices: readonly number[]): string {
  const groups: number[][] = [];
  for (const index of indices) {
    const current = groups[groups.length - 1];
    if (current && index === current[current.length - 1] + 1) {
      current.push(index);
    } else {
      groups.push([index]);
    }
  }
  return groups
    .map((group) =>
      group.length === 1 ? `${group[0]}` : `${group[0]}–${group[group.length - 1]}`,
    )
    .join(", ");
}
