import { describe, expect, it } from "vitest";
import { shownSlot, shownText } from "./text";

// U-09 of docs/features/ui-round.md, moved here with the helpers (U6 of health-ui-follow-up): agent
// text written as the CLI writes it (controls and bidirectional marks as `\u{..}`, LF kept in a
// text, `\n` in a slot). Non-Latin text is built from code points, so this file stays ASCII.

const RLO = String.fromCodePoint(0x202e);
const ZHE = String.fromCodePoint(0x416);

describe("agent text as the sheet writes it (U-09)", () => {
  it("writes U+202E as \\u{202e}, so nothing around it is reordered; the neighbours stay as they were", () => {
    const shown = shownText(`before ${RLO}evil after`);
    expect(shown).toBe("before \\u{202e}evil after");
    expect(shown).not.toContain(RLO);
  });

  it.each([
    [0x0, "\\u{0}"],
    [0x7, "\\u{7}"],
    [0xd, "\\u{d}"],
    [0x1b, "\\u{1b}"],
    [0x7f, "\\u{7f}"],
    [0x85, "\\u{85}"],
    [0x61c, "\\u{61c}"],
    [0x200e, "\\u{200e}"],
    [0x200f, "\\u{200f}"],
    [0x202a, "\\u{202a}"],
    [0x2066, "\\u{2066}"],
    [0x2069, "\\u{2069}"],
    [0x2028, "\\u{2028}"],
    [0x2029, "\\u{2029}"],
  ])("writes U+%s as its escape, lower-case hex", (code, written) => {
    expect(shownText(`a${String.fromCodePoint(code)}b`)).toBe(`a${written}b`);
  });

  it("keeps line feeds and tabs of a text, and every other character, Cyrillic included, as stored", () => {
    const text = `${ZHE}${ZHE} one\n\ttwo ${String.fromCodePoint(0xe9)} ${String.fromCodePoint(0x4e2d)}`;
    expect(shownText(text)).toBe(text);
  });

  it("writes a slot on one line: LF as \\n, CR as \\r, the rest as a text", () => {
    expect(shownSlot(`label\nnext\r${RLO}`)).toBe("label\\nnext\\r\\u{202e}");
  });
});
