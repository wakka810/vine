import type { RuntimeKeyboardInputEvent, RuntimeKeyboardSourceEventKind } from "./protocol.js";

const fixedVirtualKeys = new Map<string, number>([
  ["Backspace", 0x08],
  ["Tab", 0x09],
  ["Enter", 0x0d],
  ["NumpadEnter", 0x0d],
  ["Pause", 0x13],
  ["CapsLock", 0x14],
  ["Escape", 0x1b],
  ["Space", 0x20],
  ["PageUp", 0x21],
  ["PageDown", 0x22],
  ["End", 0x23],
  ["Home", 0x24],
  ["ArrowLeft", 0x25],
  ["ArrowUp", 0x26],
  ["ArrowRight", 0x27],
  ["ArrowDown", 0x28],
  ["PrintScreen", 0x2c],
  ["Insert", 0x2d],
  ["Delete", 0x2e],
  ["ShiftLeft", 0xa0],
  ["ShiftRight", 0xa1],
  ["ControlLeft", 0xa2],
  ["ControlRight", 0xa3],
  ["AltLeft", 0xa4],
  ["AltRight", 0xa5],
  ["MetaLeft", 0x5b],
  ["MetaRight", 0x5c],
  ["ContextMenu", 0x5d],
  ["NumpadMultiply", 0x6a],
  ["NumpadAdd", 0x6b],
  ["NumpadComma", 0x6c],
  ["NumpadSubtract", 0x6d],
  ["NumpadDecimal", 0x6e],
  ["NumpadDivide", 0x6f],
  ["NumLock", 0x90],
  ["ScrollLock", 0x91],
  ["Semicolon", 0xba],
  ["Equal", 0xbb],
  ["Comma", 0xbc],
  ["Minus", 0xbd],
  ["Period", 0xbe],
  ["Slash", 0xbf],
  ["Backquote", 0xc0],
  ["BracketLeft", 0xdb],
  ["Backslash", 0xdc],
  ["BracketRight", 0xdd],
  ["Quote", 0xde],
  ["IntlBackslash", 0xe2],
]);

export function virtualKeyForKeyboardCode(code: string): number | null {
  const fixed = fixedVirtualKeys.get(code);
  if (fixed !== undefined) {
    return fixed;
  }
  if (/^Digit[0-9]$/.test(code)) {
    return 0x30 + Number(code.at(-1));
  }
  if (/^Key[A-Z]$/.test(code)) {
    return 0x41 + (code.charCodeAt(3) - 0x41);
  }
  if (/^Numpad[0-9]$/.test(code)) {
    return 0x60 + Number(code.at(-1));
  }
  const functionMatch = /^F([1-9]|1[0-9]|2[0-4])$/.exec(code);
  if (functionMatch !== null) {
    return 0x6f + Number(functionMatch[1]);
  }
  return null;
}

// Chrome's repeat flag describes a physical key. The wire event describes
// the virtual key, which can have several physical sources (e.g. Enter).
export class BrowserKeyboardSource {
  readonly #heldCodes = new Map<string, number>();

  keyEvent(
    sequence: number,
    kind: Extract<RuntimeKeyboardSourceEventKind, "key-down" | "key-up">,
    code: string,
  ): RuntimeKeyboardInputEvent | null {
    const virtualKey = virtualKeyForKeyboardCode(code);
    if (virtualKey === null) return null;
    if (kind === "key-up" && !this.#heldCodes.delete(code)) return null;

    let held = false;
    for (const key of this.#heldCodes.values()) {
      if (key === virtualKey) {
        held = true;
        break;
      }
    }
    if (kind === "key-up" && held) return null;
    if (kind === "key-down") this.#heldCodes.set(code, virtualKey);
    return { sequence: String(sequence), kind, virtualKey, repeat: kind === "key-down" && held };
  }

  focusEvent(sequence: number, focused: boolean): RuntimeKeyboardInputEvent {
    if (!focused) this.reset();
    return {
      sequence: String(sequence),
      kind: focused ? "focus-gained" : "focus-lost",
      virtualKey: null,
      repeat: false,
    };
  }

  reset(): void {
    this.#heldCodes.clear();
  }
}
