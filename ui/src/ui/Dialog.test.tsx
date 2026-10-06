import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Announcer } from "./announcer";
import { Dialog } from "./Dialog";

// The in-app dialog of `ui/README.md` "Screen rules": modal, Esc, focus kept inside.

function isInert(element: Element): boolean {
  for (let node: Element | null = element; node !== null; node = node.parentElement) {
    if (node instanceof HTMLElement && node.inert) {
      return true;
    }
  }
  return false;
}

function part(selector: string): HTMLElement {
  const found = document.querySelector<HTMLElement>(selector);
  if (found === null) {
    throw new Error(`nothing matches ${selector}`);
  }
  return found;
}

function Page({ open, onClose, closeOnScrim }: { open: boolean; onClose: () => void; closeOnScrim?: boolean }) {
  return (
    <Announcer>
      <button type="button">Behind</button>
      {open && (
        <Dialog title="Something to decide" onClose={onClose} closeOnScrim={closeOnScrim}>
          <button type="button">Inside</button>
        </Dialog>
      )}
    </Announcer>
  );
}

function renderDialog(onClose: () => void, closeOnScrim?: boolean) {
  return render(<Page open onClose={onClose} closeOnScrim={closeOnScrim} />);
}

describe("Dialog", () => {
  it("closes on Esc pressed anywhere in its host, not only on the panel", () => {
    const onClose = vi.fn();
    renderDialog(onClose);
    fireEvent.keyDown(part("[data-dialog-host]"), { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
    fireEvent.keyDown(screen.getByRole("button", { name: "Inside" }), { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it("leaves an Esc that ends an IME composition to the input method: no close, not prevented", () => {
    const onClose = vi.fn();
    renderDialog(onClose);
    const inside = screen.getByRole("button", { name: "Inside" });
    expect(fireEvent.keyDown(inside, { key: "Escape", isComposing: true })).toBe(true);
    expect(onClose).not.toHaveBeenCalled();
    expect(fireEvent.keyDown(inside, { key: "Escape" })).toBe(false);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("keeps focus where it was when the scrim is pressed, and stays open on a click there by default", () => {
    const onClose = vi.fn();
    renderDialog(onClose);
    const inside = screen.getByRole("button", { name: "Inside" });
    expect(document.activeElement).toBe(inside);
    expect(fireEvent.mouseDown(part("[data-dialog-scrim]"))).toBe(false);
    fireEvent.click(part("[data-dialog-scrim]"));
    expect(document.activeElement).toBe(inside);
    expect(onClose).not.toHaveBeenCalled();
  });

  it("closes on a click on the scrim with closeOnScrim, never on a click inside the panel", () => {
    const onClose = vi.fn();
    renderDialog(onClose, true);
    const inside = screen.getByRole("button", { name: "Inside" });
    expect(fireEvent.mouseDown(part("[data-dialog-scrim]"))).toBe(false);
    expect(document.activeElement).toBe(inside);
    fireEvent.click(screen.getByRole("dialog"));
    fireEvent.click(inside);
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.click(part("[data-dialog-scrim]"));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("keeps Tab and Shift+Tab inside when the panel itself has focus (a click on its text)", () => {
    render(
      <Announcer>
        <button type="button">Behind</button>
        <Dialog title="Two stops" onClose={() => undefined}>
          <button type="button">First</button>
          <button type="button">Last</button>
        </Dialog>
      </Announcer>,
    );
    const panel = screen.getByRole("dialog");
    panel.focus();
    expect(document.activeElement).toBe(panel);
    fireEvent.keyDown(panel, { key: "Tab", shiftKey: true });
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Last" }));
    panel.focus();
    fireEvent.keyDown(panel, { key: "Tab" });
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "First" }));
  });

  it("makes the page inert but never the live regions, and restores the page on close", () => {
    const { rerender } = renderDialog(() => undefined);
    const behind = screen.getByRole("button", { name: "Behind" });
    const live = part('[aria-live="polite"]');
    expect(isInert(behind)).toBe(true);
    expect(isInert(live)).toBe(false);
    expect(isInert(part('[aria-live="assertive"]'))).toBe(false);
    expect(isInert(screen.getByRole("dialog"))).toBe(false);
    rerender(<Page open={false} onClose={() => undefined} />);
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(behind.isConnected).toBe(true);
    expect(isInert(behind)).toBe(false);
  });
});
