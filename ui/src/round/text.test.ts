import { describe, expect, it } from "vitest";
import { ANSWER_MAX_BYTES, REASON_MAX_BYTES, ROUND_COMMAND, utf8Bytes } from "./text";

// U-08 of docs/features/ui-round.md, the helpers: the owner's text counted in UTF-8 bytes, not
// characters or UTF-16 units (U-09, agent text as the sheet writes it, moved with its helpers to
// src/ui/text.test.ts). Non-Latin text is built from code points, so this file stays ASCII.

const ZHE = String.fromCodePoint(0x416);

describe("the owner's text in UTF-8 bytes (U-08)", () => {
  it("counts 1 025 Cyrillic letters as 2 050 bytes, over the answer's 2 048: never their 1 025 characters", () => {
    const text = ZHE.repeat(1025);
    expect([text.length, utf8Bytes(text)]).toEqual([1025, 2050]);
    expect(utf8Bytes(text)).toBeGreaterThan(ANSWER_MAX_BYTES);
  });

  it("counts a character outside the BMP as 4 bytes (2 UTF-16 units), ASCII as 1", () => {
    expect(utf8Bytes(String.fromCodePoint(0x1f600))).toBe(4);
    expect(utf8Bytes("abc")).toBe(3);
  });

  it("caps an answer at 2 048 bytes and a reason at 4 096; the round command is fixed words", () => {
    expect([ANSWER_MAX_BYTES, REASON_MAX_BYTES]).toEqual([2048, 4096]);
    expect(ROUND_COMMAND).toBe("spec round answer --staged");
  });
});
