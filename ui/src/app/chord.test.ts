import { describe, expect, it } from "vitest";
import { isJumpChord, type ChordEvent } from "./chord";

// docs/features/ui-home.md "Chord" (AC-09): Cmd-K or Ctrl-K, the physical K only where `key` is
// not one ASCII letter; never with Alt, Shift or during composition.

function press(fields: Partial<ChordEvent>): boolean {
  return isJumpChord({ key: "k", code: "KeyK", metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, isComposing: false, ...fields });
}

describe("the palette's chord", () => {
  it("is Cmd-K or Ctrl-K, either case of K", () => {
    expect([press({ metaKey: true }), press({ ctrlKey: true }), press({ key: "K", ctrlKey: true })]).toEqual([true, true, true]);
  });

  it("reads the physical K on a layout whose key is no ASCII letter (Cyrillic)", () => {
    expect(press({ key: "\u043b", code: "KeyK", metaKey: true })).toBe(true);
  });

  it("reads the typed letter on a Latin layout: Dvorak's Cmd-T on KeyK is not K", () => {
    expect(press({ key: "t", code: "KeyK", metaKey: true })).toBe(false);
  });

  it("is K typed anywhere, whatever the physical key", () => {
    expect(press({ key: "k", code: "KeyV", ctrlKey: true })).toBe(true);
  });

  it("is nothing without Cmd or Ctrl, or with Alt, Shift or during composition", () => {
    expect([
      press({}),
      press({ metaKey: true, altKey: true }),
      press({ ctrlKey: true, shiftKey: true }),
      press({ metaKey: true, isComposing: true }),
      press({ key: "j", code: "KeyJ", metaKey: true }),
      press({ key: "\u043b", code: "KeyL", metaKey: true }),
    ]).toEqual([false, false, false, false, false, false]);
  });
});
