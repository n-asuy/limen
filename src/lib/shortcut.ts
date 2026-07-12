type ParseOptions = {
  requireModifier?: boolean;
};

type ParseComplete = {
  status: "complete";
  accelerator: string;
  display: string;
};

/** Modifiers are held but no main key has been pressed yet. */
type ParsePending = {
  status: "pending";
  display: string;
};

type ParseInvalid = {
  status: "invalid";
  reason: string;
};

export type ShortcutParseResult = ParseComplete | ParsePending | ParseInvalid;

const SPECIAL_SYMBOLS: Record<string, string> = {
  Command: "⌘",
  Control: "⌃",
  Alt: "⌥",
  Option: "⌥",
  Shift: "⇧",
  Super: "⌘",
};

const KEY_DISPLAY_MAP: Record<string, string> = {
  Space: "Space",
  Tab: "⇥",
  Enter: "⏎",
  Return: "⏎",
  Escape: "Esc",
  Backspace: "⌫",
  Delete: "⌦",
  ArrowUp: "↑",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
  PageUp: "PgUp",
  PageDown: "PgDn",
  Home: "Home",
  End: "End",
  Insert: "Ins",
};

const SYMBOL_KEY_MAP: Record<string, string> = {
  Minus: "-",
  Equal: "=",
  Backquote: "`",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Semicolon: ";",
  Quote: "'",
};

const MODIFIER_CODES = new Set([
  "ShiftLeft",
  "ShiftRight",
  "MetaLeft",
  "MetaRight",
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
]);

const DISALLOWED_CODES = new Set(["CapsLock", "NumLock", "ScrollLock"]);

const FUNCTION_KEY_PATTERN = /^F([0-2]?\d)$/i;

export function isLikelyMac(): boolean {
  if (typeof navigator === "undefined") return false;
  return /Mac/i.test(navigator.platform || navigator.userAgent);
}

export function formatAccelerator(accelerator: string): string {
  if (!accelerator) return "";
  const parts = accelerator
    .split("+")
    .map((part) => part.trim())
    .filter(Boolean);
  if (parts.length === 0) return "";

  const mac = isLikelyMac();
  return parts
    .map((part, index) => {
      if (index < parts.length - 1) {
        return displayForModifier(part, mac);
      }
      return displayForKey(part);
    })
    .join(" ");
}

function displayForModifier(part: string, mac: boolean): string {
  if (SPECIAL_SYMBOLS[part]) return SPECIAL_SYMBOLS[part];
  const upper = part.toUpperCase();
  if (upper === "CMD" || upper === "COMMAND" || upper === "SUPER") {
    return mac ? "⌘" : "Win";
  }
  if (upper === "ALT" || upper === "OPTION") return SPECIAL_SYMBOLS.Option;
  if (upper === "CTRL" || upper === "CONTROL") return mac ? "⌃" : "Ctrl";
  if (upper === "SHIFT") return mac ? "⇧" : "Shift";
  return part;
}

/**
 * Convert a DOM keydown event into an accelerator string that the backend
 * shortcut parser accepts (e.g. "Option+Space", "Command+Shift+K").
 *
 * Pressing a modifier by itself is a normal step towards a combination, so it
 * yields a "pending" result (with a display of the held modifiers) instead of
 * an error.
 */
export function parseKeyboardEventToAccelerator(
  event: KeyboardEvent,
  options?: ParseOptions,
): ShortcutParseResult {
  const { requireModifier = true } = options ?? {};

  const mac = isLikelyMac();
  const orderedModifiers: string[] = [];
  if (event.metaKey) orderedModifiers.push("Command");
  if (event.ctrlKey) orderedModifiers.push("Control");
  if (event.altKey) orderedModifiers.push("Alt");
  if (event.shiftKey) orderedModifiers.push("Shift");

  if (MODIFIER_CODES.has(event.code)) {
    return {
      status: "pending",
      display: orderedModifiers.map((part) => displayForModifier(part, mac)).join(" "),
    };
  }

  if (requireModifier && orderedModifiers.length === 0) {
    return { status: "invalid", reason: "Include at least one modifier key" };
  }

  const keyToken = normalizeKeyToken(event.code, event.key);
  if (!keyToken) {
    return { status: "invalid", reason: "This key cannot be used in a shortcut" };
  }

  const accelerator = [...orderedModifiers, keyToken].join("+");

  return {
    status: "complete",
    accelerator,
    display: formatAccelerator(accelerator),
  };
}

export function humanizeShortcutError(message: string): string {
  const lower = message.toLowerCase();
  if (lower.includes("modifier")) return "Include at least one modifier key";
  if (lower.includes("invalid shortcut")) return "This shortcut cannot be registered";
  if (lower.includes("failed to register shortcut")) {
    return "Could not register the shortcut. Another app may already use it";
  }
  if (lower.includes("failed to clear shortcuts")) return "Failed to re-register shortcuts";
  return message;
}

function normalizeKeyToken(code: string, key: string): string | null {
  if (!code && !key) return null;
  if (DISALLOWED_CODES.has(code)) return null;

  if (code.startsWith("Key")) {
    return code.slice(3).toUpperCase();
  }

  if (code.startsWith("Digit")) {
    return code.slice(5);
  }

  if (code.startsWith("Numpad")) {
    const suffix = code.slice(6);
    if (/^\d$/.test(suffix)) {
      return `Num${suffix}`;
    }
    return code;
  }

  const functionMatch = code.match(FUNCTION_KEY_PATTERN);
  if (functionMatch) {
    return `F${functionMatch[1]}`;
  }

  if (SYMBOL_KEY_MAP[code]) {
    return SYMBOL_KEY_MAP[code];
  }

  switch (code) {
    case "IntlBackslash":
      return "\\";
    case "Space":
    case "Tab":
    case "Enter":
    case "Escape":
    case "Backspace":
    case "Delete":
    case "ArrowUp":
    case "ArrowDown":
    case "ArrowLeft":
    case "ArrowRight":
    case "Home":
    case "End":
    case "PageUp":
    case "PageDown":
    case "Insert":
      return code;
    default:
      break;
  }

  if (key && key.length === 1) {
    return key.toUpperCase();
  }

  return null;
}

function displayForKey(token: string): string {
  if (!token) return "";
  const upper = token.toUpperCase();

  if (KEY_DISPLAY_MAP[token]) return KEY_DISPLAY_MAP[token];
  if (token.startsWith("Num")) return upper;
  if (upper.length === 1) return upper;
  if (FUNCTION_KEY_PATTERN.test(token)) return upper;

  return token;
}
