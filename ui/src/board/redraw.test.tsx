import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { aCardList, aCardRow } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient } from "../test/stubClient";
import { isOverdue } from "./due";

// What a key and a tick redraw (docs/features/board-ui.md "Moving"): a card calls `isOverdue` once
// per render, so its calls count the cards drawn.
vi.mock("./due", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./due")>();
  return { ...actual, isOverdue: vi.fn(actual.isOverdue) };
});

const CARDS = 30;

async function fullBoard() {
  const client = stubClient();
  client.getCards.mockResolvedValue(aCardList(Array.from({ length: CARDS }, (_, index) => aCardRow({ id: `C-${String(1000 + index)}`, due: "2999-01-01" }))));
  renderApp(client, "#/alpha/board");
  await waitFor(() => {
    expect(document.querySelectorAll("[data-card]")).toHaveLength(CARDS);
  });
  const first = document.querySelector<HTMLElement>('[data-card="C-1000"]');
  if (first === null) {
    throw new Error("no card");
  }
  return first;
}

describe("what the board redraws", () => {
  beforeEach(() => {
    vi.mocked(isOverdue).mockClear();
  });

  it("an arrow key redraws the two cards the tab stop leaves and reaches, not the board", async () => {
    const first = await fullBoard();
    first.focus();
    vi.mocked(isOverdue).mockClear();
    fireEvent.keyDown(first, { key: "ArrowDown" });
    await waitFor(() => {
      expect(document.activeElement?.getAttribute("data-card")).toBe("C-1001");
    });
    expect(vi.mocked(isOverdue).mock.calls.length).toBeLessThanOrEqual(2);
  });

  it("typing the acting name redraws no card", async () => {
    await fullBoard();
    vi.mocked(isOverdue).mockClear();
    fireEvent.change(screen.getByLabelText("Acting as"), { target: { value: "Mira Holt" } });
    expect(isOverdue).not.toHaveBeenCalled();
  });

  it("a clock tick redraws nothing within a day, every card when the day turns", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "Date"], now: new Date(2026, 2, 1, 12, 0) });
    await fullBoard();
    vi.mocked(isOverdue).mockClear();
    act(() => {
      vi.advanceTimersByTime(30_000);
    });
    expect(isOverdue).not.toHaveBeenCalled();
    vi.setSystemTime(new Date(2026, 2, 2, 0, 0, 5));
    act(() => {
      vi.advanceTimersByTime(30_000);
    });
    expect(vi.mocked(isOverdue).mock.calls.length).toBeGreaterThanOrEqual(CARDS);
  });
});
