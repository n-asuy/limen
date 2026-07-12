export type SpaceApp = {
  bundleId: string;
  name: string;
  count: number;
  lastSeen: number;
};

export type SpaceAppsMap = Record<string, Record<string, SpaceApp>>; // spaceId -> bundleId -> data

export function topAppsPure(spaceApps: SpaceAppsMap, spaceId: number, n = 3) {
  const bucket = spaceApps[String(spaceId)] || {};
  return Object.values(bucket)
    // 純粋に直近順（lastSeen 降順）
    .sort((a, b) => b.lastSeen - a.lastSeen)
    .slice(0, n);
}
