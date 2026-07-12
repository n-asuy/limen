import { describe, it, expect } from "bun:test";
import {
  topAppsPure,
  type SpaceAppsMap,
} from "../lib/space-utils";

describe("topAppsPure", () => {
  const apps: SpaceAppsMap = {
    "1": {
      a: { bundleId: "a", name: "A", count: 2, lastSeen: 100 },
      b: { bundleId: "b", name: "B", count: 5, lastSeen: 90 },
      c: { bundleId: "c", name: "C", count: 5, lastSeen: 110 },
    },
  };

  // 仕様: lastSeen 降順で上位N件を返す
  it("sorts by lastSeen desc and slices N", () => {
    const top2 = topAppsPure(apps, 1, 2);
    expect(top2.map((x) => x.bundleId)).toEqual(["c", "a"]);
  });

  // 仕様: 対象 spaceId が存在しない場合は空配列を返す
  it("returns empty for missing space", () => {
    expect(topAppsPure({}, 99, 3)).toEqual([]);
  });
});
