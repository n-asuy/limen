import { describe, it, expect } from "bun:test";
import {
  parseKeyboardEventToAccelerator,
  formatAccelerator,
  humanizeShortcutError,
} from "../lib/shortcut";

const keyEvent = (init: Partial<KeyboardEvent>): KeyboardEvent =>
  ({
    metaKey: false,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    code: "",
    key: "",
    ...init,
  }) as KeyboardEvent;

describe("parseKeyboardEventToAccelerator", () => {
  // 仕様: 修飾キー+キーをバックエンドが受理する書式の文字列に変換する
  it("builds an accelerator from modifiers and key", () => {
    const result = parseKeyboardEventToAccelerator(
      keyEvent({ altKey: true, code: "Space", key: " " }),
    );
    expect(result.status).toBe("complete");
    if (result.status === "complete") expect(result.accelerator).toBe("Alt+Space");
  });

  // 仕様: 修飾キーは Command → Control → Alt → Shift の順に正規化される
  it("orders modifiers deterministically", () => {
    const result = parseKeyboardEventToAccelerator(
      keyEvent({ shiftKey: true, metaKey: true, code: "KeyK", key: "k" }),
    );
    expect(result.status).toBe("complete");
    if (result.status === "complete") expect(result.accelerator).toBe("Command+Shift+K");
  });

  // 仕様: 修飾キー必須オプション下では修飾キー無しを拒否する
  it("rejects bare keys when a modifier is required", () => {
    const result = parseKeyboardEventToAccelerator(keyEvent({ code: "KeyK", key: "k" }));
    expect(result.status).toBe("invalid");
  });

  // 仕様: 修飾キー単体はエラーではなく「押しかけ (pending)」として扱う
  it("treats modifier-only presses as pending, not invalid", () => {
    const result = parseKeyboardEventToAccelerator(
      keyEvent({ shiftKey: true, code: "ShiftLeft", key: "Shift" }),
    );
    expect(result.status).toBe("pending");
    if (result.status === "pending") expect(result.display.length).toBeGreaterThan(0);
  });

  // 仕様: 複数修飾キーの押しかけも pending として保持中の修飾キーを表示する
  it("reports all held modifiers while pending", () => {
    const result = parseKeyboardEventToAccelerator(
      keyEvent({ metaKey: true, shiftKey: true, code: "MetaLeft", key: "Meta" }),
    );
    expect(result.status).toBe("pending");
    if (result.status === "pending") {
      expect(result.display.split(" ")).toHaveLength(2);
    }
  });

  // 仕様: ロック系キー (CapsLock 等) はショートカットに使えない
  it("rejects lock keys as invalid", () => {
    const result = parseKeyboardEventToAccelerator(
      keyEvent({ shiftKey: true, code: "CapsLock", key: "CapsLock" }),
    );
    expect(result.status).toBe("invalid");
  });

  // 仕様: 数字キーは Digit コードから数字トークンへ変換する
  it("maps digit codes to plain digits", () => {
    const result = parseKeyboardEventToAccelerator(
      keyEvent({ ctrlKey: true, code: "Digit3", key: "3" }),
    );
    expect(result.status).toBe("complete");
    if (result.status === "complete") expect(result.accelerator).toBe("Control+3");
  });
});

describe("formatAccelerator", () => {
  // 仕様: mac では修飾キーを記号で表示する(navigator が Mac でない環境ではテキスト)
  it("renders a readable label", () => {
    const label = formatAccelerator("Alt+Space");
    expect(label.endsWith("Space")).toBe(true);
    expect(label.split(" ")).toHaveLength(2);
  });

  // 仕様: 空文字は空文字のまま
  it("returns empty for empty input", () => {
    expect(formatAccelerator("")).toBe("");
  });
});

describe("humanizeShortcutError", () => {
  // 仕様: バックエンドのエラー文を利用者向けの文に変換する
  it("translates known backend errors", () => {
    expect(humanizeShortcutError("invalid shortcut: unknown key")).toBe(
      "This shortcut cannot be registered",
    );
    expect(
      humanizeShortcutError("failed to register shortcut Alt+Space: already in use"),
    ).toContain("Another app");
  });

  // 仕様: 未知のエラーはそのまま返す
  it("passes through unknown messages", () => {
    expect(humanizeShortcutError("boom")).toBe("boom");
  });
});
