import { describe, it, expect } from "bun:test";
import {
  SPACE_SHORTCUT_COUNT,
  describeSpaceShortcuts,
  formatSpaceRanges,
  summarizeSpaceShortcuts,
} from "../lib/space-shortcuts";

describe("summarizeSpaceShortcuts", () => {
  // 仕様: 1-9 すべて有効なら ready で、不足はない
  it("reports ready when every Space shortcut is available", () => {
    const summary = summarizeSpaceShortcuts([1, 2, 3, 4, 5, 6, 7, 8, 9]);
    expect(summary.status).toBe("ready");
    expect(summary.missing).toEqual([]);
    expect(summary.enabled).toHaveLength(SPACE_SHORTCUT_COUNT);
  });

  // 仕様: 1つも有効でないなら disabled で、Space切り替えは一切効かない
  it("reports disabled when no Space shortcut is available", () => {
    const summary = summarizeSpaceShortcuts([]);
    expect(summary.status).toBe("disabled");
    expect(summary.enabled).toEqual([]);
    expect(summary.missing).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9]);
  });

  // 仕様: 一部だけ有効なら partial で、効かない Space を列挙する
  it("reports partial with the Spaces that will not switch", () => {
    const summary = summarizeSpaceShortcuts([1, 2, 3, 4]);
    expect(summary.status).toBe("partial");
    expect(summary.enabled).toEqual([1, 2, 3, 4]);
    expect(summary.missing).toEqual([5, 6, 7, 8, 9]);
  });

  // 仕様: 範囲外・重複・未整列の入力を正規化する
  it("normalizes out-of-range, duplicate and unsorted input", () => {
    const summary = summarizeSpaceShortcuts([9, 0, 3, 3, 10, -1]);
    expect(summary.enabled).toEqual([3, 9]);
    expect(summary.missing).toEqual([1, 2, 4, 5, 6, 7, 8]);
    expect(summary.status).toBe("partial");
  });
});

describe("describeSpaceShortcuts", () => {
  // 仕様: どの状態でも、リング操作と macOS 設定の関係を説明する
  // ("Mission Control shortcuts" というラベルだけでは何の話か伝わらない)
  it("ties the ring to the macOS shortcut in every state", () => {
    const states = [[1, 2, 3, 4, 5, 6, 7, 8, 9], [1, 2, 3, 4], [], null];
    for (const available of states) {
      const summary = available ? summarizeSpaceShortcuts(available) : null;
      expect(describeSpaceShortcuts(summary)).toContain("Picking a Space in the ring");
    }
  });

  // 仕様: ready なら手順は出さない(やることが無い)
  it("omits the setup steps when there is nothing to do", () => {
    const message = describeSpaceShortcuts(summarizeSpaceShortcuts([1, 2, 3, 4, 5, 6, 7, 8, 9]));
    expect(message).not.toContain("System Settings");
  });

  // 仕様: partial なら効かない Space を名指しし、何が起きるか(無反応)と手順を示す
  it("names the Spaces that will not switch and how to fix them", () => {
    const message = describeSpaceShortcuts(summarizeSpaceShortcuts([1, 2, 3, 4]));
    expect(message).toContain("Spaces 5–9");
    expect(message).toContain("does nothing");
    expect(message).toContain("Mission Control");
  });

  // 仕様: disabled なら既定で無効である事実・切り替わらない結果・手順を示す
  it("explains the macOS default, the consequence and the setup steps", () => {
    const message = describeSpaceShortcuts(summarizeSpaceShortcuts([]));
    expect(message).toContain("disabled by default");
    expect(message).toContain("no Space will switch");
    expect(message).toContain("Mission Control");
  });

  // 仕様: 状態不明(検出失敗)でも手順は案内する
  it("falls back to the setup steps when the status is unknown", () => {
    const message = describeSpaceShortcuts(null);
    expect(message).toContain("Mission Control");
  });
});

describe("formatSpaceRanges", () => {
  // 仕様: 連続する番号は範囲に畳む
  it("collapses consecutive indices into a range", () => {
    expect(formatSpaceRanges([5, 6, 7, 8, 9])).toBe("5–9");
  });

  // 仕様: 飛び番号は区切って並べ、2連番は範囲にする
  it("joins disjoint groups", () => {
    expect(formatSpaceRanges([2, 5, 6])).toBe("2, 5–6");
  });

  // 仕様: 単独の番号はそのまま
  it("prints a lone index as-is", () => {
    expect(formatSpaceRanges([4])).toBe("4");
  });

  // 仕様: 空なら空文字
  it("returns an empty string for no indices", () => {
    expect(formatSpaceRanges([])).toBe("");
  });
});
