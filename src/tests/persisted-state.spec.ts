import { describe, it, expect } from "bun:test";
import {
  interpretPersistedText,
  buildDefaultSpaces,
  ensureSpaceCount,
  cleanIconPaths,
} from "../lib/persisted-state";

describe("interpretPersistedText", () => {
  const validState = JSON.stringify({
    spaces: [{ id: 1, name: "Work", visited: true, order: 0 }],
    spaceApps: {},
    appIcons: {},
  });

  // 仕様: 空文字は「ファイル無し」= デフォルト初期化して保存してよい
  it("treats empty text as missing", () => {
    expect(interpretPersistedText("").kind).toBe("missing");
    expect(interpretPersistedText("   ").kind).toBe("missing");
  });

  // 仕様: 正常な保存データは ok として中身を返す
  it("returns ok with state for valid payload", () => {
    const result = interpretPersistedText(validState);
    expect(result.kind).toBe("ok");
    if (result.kind === "ok") {
      expect(result.state.spaces[0].name).toBe("Work");
    }
  });

  // 仕様: 壊れたJSONは invalid（呼び出し側は退避せずに上書きしてはならない）
  it("flags broken JSON as invalid, never missing", () => {
    expect(interpretPersistedText("{ oops").kind).toBe("invalid");
  });

  // 仕様: JSONとして正しくても形が違えば invalid（上書き禁止）
  it("flags shape mismatches as invalid", () => {
    expect(interpretPersistedText('{"spaces":"not-array"}').kind).toBe("invalid");
    expect(interpretPersistedText('{"spaces":[],"spaceApps":[],"appIcons":{}}').kind).toBe(
      "invalid",
    );
    expect(interpretPersistedText('{"spaces":[],"spaceApps":{},"appIcons":null}').kind).toBe(
      "invalid",
    );
    expect(interpretPersistedText("[1,2,3]").kind).toBe("invalid");
  });
});

describe("buildDefaultSpaces", () => {
  // 仕様: id は 1 始まり、order は 0 始まり、名前は Space N
  it("builds sequential defaults", () => {
    const spaces = buildDefaultSpaces(9);
    expect(spaces).toHaveLength(9);
    expect(spaces[0]).toMatchObject({ id: 1, name: "Space 1", order: 0, visited: false });
    expect(spaces[8]).toMatchObject({ id: 9, name: "Space 9", order: 8 });
  });
});

describe("ensureSpaceCount", () => {
  // 仕様: 保存済みデータはそのまま残し、不足分だけデフォルトで埋める
  it("pads while preserving existing entries", () => {
    const saved = [
      { id: 1, name: "Work", visited: true, order: 0 },
      { id: 2, name: "Play", visited: false, order: 1 },
    ];
    const { spaces, changed } = ensureSpaceCount(saved as any, 9);
    expect(changed).toBe(true);
    expect(spaces).toHaveLength(9);
    expect(spaces[0].name).toBe("Work");
    expect(spaces[2]).toMatchObject({ id: 3, name: "Space 3", order: 2 });
  });

  // 仕様: 既に足りている場合は変更なし（同一参照を返す）
  it("does nothing when already enough", () => {
    const saved = buildDefaultSpaces(9);
    const { spaces, changed } = ensureSpaceCount(saved, 9);
    expect(changed).toBe(false);
    expect(spaces).toBe(saved);
  });
});

describe("cleanIconPaths", () => {
  // 仕様: ファイルパスに見える値はそのまま
  it("keeps path-like values", () => {
    const icons = { "com.example.app": "/Users/x/icons/com.example.app.png" };
    const { icons: kept, changed } = cleanIconPaths(icons);
    expect(changed).toBe(false);
    expect(kept).toBe(icons);
  });

  // 仕様: 旧形式（base64等）が混ざっていたら全部捨てて作り直す
  it("drops the whole map when a legacy value is present", () => {
    const icons = {
      good: "/tmp/good.png",
      legacy: "data:image/png;base64,AAAA",
    };
    const { icons: kept, changed } = cleanIconPaths(icons);
    expect(changed).toBe(true);
    expect(kept).toEqual({});
  });
});
