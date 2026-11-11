import { fireEvent } from "@testing-library/react";
import { vi } from "vitest";

// A press on a React Flow canvas in jsdom, shared by Flow's, Graph's and the Chain's tests
// (docs/features/handoff-progress.md "Acceptance criteria", C-1, C-3; docs/features/canvas-clicks.md
// "Acceptance criteria", CK1, CK2, CK5). d3-zoom follows a press on its event's `view`: it pans past
// the canvas's click distance and then swallows the click on that view. jsdom's event constructors
// refuse this test run's window there, so the property is set on the event made without it.

/** A mouse or pointer event with the window as its `view`, as a browser sends it. */
export function withView<T extends Event>(event: T): T {
  Object.defineProperty(event, "view", { value: document.defaultView });
  return event;
}

/** A mouse event of `type` on `target` at (x, y), primary button, with its view. */
export function mouse(target: Element, type: string, x: number, y: number) {
  fireEvent(target, withView(new MouseEvent(type, { bubbles: true, cancelable: true, button: 0, clientX: x, clientY: y })));
}

/** A pointer event of `type` on `target`, primary button, with its view; a mouse's unless `pointerType` says. */
export function pointer(target: Element, type: string, pointerType = "mouse") {
  fireEvent(target, withView(new PointerEvent(type, { bubbles: true, cancelable: true, button: 0, pointerType })));
}

/** A mouse press on `target` at (100, 100), a move by (dx, dy), its release and its click. */
export function pressAndMove(target: Element, dx: number, dy: number) {
  mouse(target, "mousedown", 100, 100);
  mouse(target, "mousemove", 100 + dx, 100 + dy);
  mouse(target, "mouseup", 100 + dx, 100 + dy);
  mouse(target, "click", 100 + dx, 100 + dy);
}

/** The canvas's pan and zoom as written, from the viewport's transform. */
export function viewOf(): string {
  return document.querySelector<HTMLElement>(".react-flow__viewport")?.style.transform ?? "";
}

/**
 * React Flow's zoom pane measured `width` x `height`, as a browser lays it out; every other element
 * measures 0, as jsdom does. Returns the restore.
 */
export function stubCanvas(width: number, height: number): () => void {
  const before = {
    width: Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetWidth"),
    height: Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetHeight"),
  };
  Object.defineProperty(HTMLElement.prototype, "offsetWidth", {
    configurable: true,
    get(this: HTMLElement) {
      return this.classList.contains("react-flow__renderer") ? width : 0;
    },
  });
  Object.defineProperty(HTMLElement.prototype, "offsetHeight", {
    configurable: true,
    get(this: HTMLElement) {
      return this.classList.contains("react-flow__renderer") ? height : 0;
    },
  });
  return () => {
    for (const [name, descriptor] of [
      ["offsetWidth", before.width],
      ["offsetHeight", before.height],
    ] as const) {
      if (descriptor !== undefined) {
        Object.defineProperty(HTMLElement.prototype, name, descriptor);
      }
    }
  };
}

/** Whether the focused element matches `:focus-visible`, as `stubFocusVisible` answers it. */
export interface FocusVisible {
  /** True: a focus from the keyboard, as a browser marks it. */
  keyboard: boolean;
  restore: () => void;
}

/**
 * jsdom never matches `:focus-visible`; a browser does for a focus that came from the keyboard. Here
 * the focused element matches it while `keyboard` is true; every other selector is jsdom's own.
 */
export function stubFocusVisible(): FocusVisible {
  const own = Object.getOwnPropertyDescriptor(Element.prototype, "matches")?.value as (this: Element, selector: string) => boolean;
  const state: FocusVisible = { keyboard: false, restore: () => undefined };
  const spy = vi.spyOn(Element.prototype, "matches").mockImplementation(function (this: Element, selector: string) {
    return selector === ":focus-visible" ? state.keyboard && this === document.activeElement : own.call(this, selector);
  });
  state.restore = () => {
    spy.mockRestore();
  };
  return state;
}
