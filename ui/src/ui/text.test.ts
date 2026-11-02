import { describe, expect, it } from "vitest";
import { shownConfirmSlot, shownConfirmText, shownSlot, shownText } from "./text";

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

// B12 of docs/features/hub-confirm.md, Escaping in `docs/canon/hub-confirm.md` "Rules": a Confirm
// dialog writes every format character (Cf) as `\u{..}`, as the daemon's confirm question does; the
// CLI's own escape is unchanged.

const CATEGORY_CF = /^\p{Cf}$/u;

describe("a confirm's text (hub-confirm B12)", () => {
  it.each([
    [0xad, "\\u{ad}"],
    [0x200b, "\\u{200b}"],
    [0x2060, "\\u{2060}"],
    [0xfeff, "\\u{feff}"],
    [0x200c, "\\u{200c}"],
    [0x200d, "\\u{200d}"],
    [0x600, "\\u{600}"],
    [0xe0001, "\\u{e0001}"],
    [0x202e, "\\u{202e}"],
    [0x7, "\\u{7}"],
    [0x2028, "\\u{2028}"],
  ])("writes U+%s as its escape, lower-case hex", (code, written) => {
    expect(shownConfirmText(`a${String.fromCodePoint(code)}b`)).toBe(`a${written}b`);
  });

  it("leaves the CLI's escape as it is: a joiner, a zero-width space, U+FEFF and a soft hyphen stay as stored there", () => {
    const text = [0x200c, 0x200d, 0x200b, 0xfeff, 0xad, 0x2060].map((code) => `x${String.fromCodePoint(code)}`).join("");
    expect(shownText(text)).toBe(text);
  });

  it("keeps line feeds, tabs, Cyrillic and a written escape; a slot writes LF and CR", () => {
    const text = `${ZHE} one\n\ttwo \\u{200b}`;
    expect(shownConfirmText(text)).toBe(text);
    expect(shownConfirmSlot(`a\nb\r${String.fromCodePoint(0x200b)}`)).toBe("a\\nb\\r\\u{200b}");
  });

  it("escapes exactly category Cf beyond the CLI's set: the table matches this runtime's Unicode", () => {
    const beyond: string[] = [];
    const missed: string[] = [];
    for (let code = 0; code <= 0x10ffff; code += 1) {
      if (code >= 0xd800 && code <= 0xdfff) {
        continue;
      }
      const char = String.fromCodePoint(code);
      const format = CATEGORY_CF.test(char);
      if (!format && shownConfirmText(char) !== shownText(char)) {
        beyond.push(code.toString(16));
      }
      if (format && shownConfirmText(char) === char) {
        missed.push(code.toString(16));
      }
    }
    expect({ beyond, missed }).toEqual({ beyond: [], missed: [] });
  });
});
