import { afterEach, describe, expect, it, vi } from "vitest";
import { ACTING_KEY, actingName, isActingLine, recallActing, rememberActing } from "./acting";

// The acting name (docs/features/board-ui.md "The acting name, and acceptance"): per viewer in
// `localStorage`, sent trimmed, empty as null. Any script, inner spaces kept: the daemon judges it.

describe("the acting name", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    rememberActing("");
  });

  it("is kept under its key, an empty one removed", () => {
    expect(ACTING_KEY).toBe("specengine.board.as");
    expect(recallActing()).toBe("");
    rememberActing("Mira Holt");
    expect(window.localStorage.getItem(ACTING_KEY)).toBe("Mira Holt");
    expect(recallActing()).toBe("Mira Holt");
    rememberActing("");
    expect(window.localStorage.getItem(ACTING_KEY)).toBeNull();
  });

  it("goes out trimmed, inner spaces and any script kept; none is null, which the daemon records local", () => {
    expect(actingName("  Mira Holt ")).toBe("Mira Holt");
    expect(actingName("\u0410\u043d\u043d\u0430 \u041f\u0435\u0442\u0440\u043e\u0432\u0430")).toBe("\u0410\u043d\u043d\u0430 \u041f\u0435\u0442\u0440\u043e\u0432\u0430");
    expect(actingName("   ")).toBeNull();
    expect(actingName("")).toBeNull();
  });

  it("takes a 400 naming --as for its field; any other line stays where the write was made", () => {
    expect(isActingLine({ status: 400, message: "spec: --as: 129 bytes; at most 128" })).toBe(true);
    expect(isActingLine({ status: 400, message: "spec: assignee: 129 bytes; at most 128" })).toBe(false);
    expect(isActingLine({ status: 409, message: "C-0001: its reviewer is `a`, and `--as` says `b`; nothing changed" })).toBe(false);
  });

  it("keeps each keystroke when only the write is refused, until the storage takes one again", () => {
    window.localStorage.setItem(ACTING_KEY, "stored before");
    const refused = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("refused");
    });
    rememberActing("I");
    expect(recallActing()).toBe("I");
    rememberActing("Iv");
    expect(recallActing()).toBe("Iv");
    refused.mockRestore();
    rememberActing("Ivo");
    expect(recallActing()).toBe("Ivo");
    expect(window.localStorage.getItem(ACTING_KEY)).toBe("Ivo");
  });

  it("lasts the page when the browser refuses its storage", () => {
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("refused");
    });
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("refused");
    });
    rememberActing("Ivo Brandt");
    expect(recallActing()).toBe("Ivo Brandt");
  });
});
