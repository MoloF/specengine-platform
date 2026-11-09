import { describe, expect, it } from "vitest";
import { hotkeyOf, letterOf } from "./keys";

// LG-7 of docs/features/ui-languages.md: a letter hotkey reads `letterOf`: the `key` when it is one
// ASCII letter; when it is one non-ASCII character, the physical `KeyA`..`KeyZ`'s letter, upper case
// with Shift; else nothing.
// Russian letters are written as escapes (D13): U+043E is the letter KeyJ types under a Russian layout.

const event = (key: string, code: string, shiftKey = false) => ({ key, code, shiftKey });

describe("letterOf (LG-7)", () => {
  it("takes an ASCII letter from key, its case as typed", () => {
    expect(letterOf(event("j", "KeyJ"))).toBe("j");
    expect(letterOf(event("J", "KeyJ", true))).toBe("J");
  });

  it("takes the physical key under a Russian layout, upper case with Shift", () => {
    expect(letterOf(event("\u043e", "KeyJ"))).toBe("j");
    expect(letterOf(event("\u041e", "KeyJ", true))).toBe("J");
    expect(letterOf(event("\u0441", "KeyC"))).toBe("c");
    expect(letterOf(event("\u0442", "KeyN"))).toBe("n");
    expect(letterOf(event("\u044c", "KeyM"))).toBe("m");
  });

  it("keeps a Latin layout's own letter: Dvorak's KeyJ types h, and h it is (M19)", () => {
    expect(letterOf(event("h", "KeyJ"))).toBe("h");
  });

  it("reads no letter from ASCII punctuation on a letter key: AZERTY's , on KeyM, Dvorak's ' on KeyQ", () => {
    expect(letterOf(event(",", "KeyM"))).toBeNull();
    expect(letterOf(event("'", "KeyQ"))).toBeNull();
    expect(letterOf(event("\u044c", "KeyM"))).toBe("m");
    expect(hotkeyOf(event(",", "KeyM"))).toBe(",");
  });

  it("reads no letter from a named key on a letter key, as a dead key or an IME's Process", () => {
    expect(letterOf(event("Dead", "KeyU"))).toBeNull();
    expect(letterOf(event("Process", "KeyJ"))).toBeNull();
  });

  it("gives nothing for a key that is no letter", () => {
    expect(letterOf(event("?", "Digit7", true))).toBeNull();
    expect(letterOf(event("1", "Digit1"))).toBeNull();
    expect(letterOf(event("\u0441", "Unidentified"))).toBeNull();
    expect(letterOf(event("ArrowDown", "ArrowDown"))).toBeNull();
    expect(letterOf(event(" ", "Space"))).toBeNull();
  });

  it("leaves every other key on key for a hotkey: ?, Escape, Enter, Space, arrows, Home, End", () => {
    expect(["?", "Escape", "Enter", " ", "ArrowUp", "Home", "End"].map((key) => hotkeyOf(event(key, key === "?" ? "Slash" : key)))).toEqual(["?", "Escape", "Enter", " ", "ArrowUp", "Home", "End"]);
    expect(hotkeyOf(event("\u043e", "KeyJ"))).toBe("j");
  });
});
