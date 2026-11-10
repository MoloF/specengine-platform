import { useEffect, useId, useRef, type KeyboardEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { LIVE_REGION_ATTRIBUTE } from "./announcer";

const FOCUSABLE = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

/**
 * Whether an element of the dialog can take focus as it is drawn: not inside a `hidden` or `inert`
 * region (a collapsed disclosure keeps its links in the DOM), not `display: none` or
 * `visibility: hidden`, and, where the page is laid out at all, with a box of its own. jsdom lays
 * nothing out, so there the boxes say nothing and the first checks alone decide.
 */
export function isTabStop(element: HTMLElement, root: HTMLElement): boolean {
  if (element.closest("[hidden], [inert]") !== null) {
    return false;
  }
  const style = window.getComputedStyle(element);
  if (style.display === "none" || style.visibility === "hidden") {
    return false;
  }
  return element.getClientRects().length > 0 || root.getClientRects().length === 0;
}

/** The dialog's tab stops in order, those drawn alone; a radio group is one stop, its checked radio (else its first). */
export function focusables(root: HTMLElement): HTMLElement[] {
  const radios = Array.from(root.querySelectorAll<HTMLInputElement>('input[type="radio"]'));
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE)).filter((element) => {
    if (!isTabStop(element, root)) {
      return false;
    }
    if (!(element instanceof HTMLInputElement) || element.type !== "radio" || element.name === "") {
      return true;
    }
    const group = radios.filter((radio) => radio.name === element.name);
    return element === (group.find((radio) => radio.checked) ?? group[0]);
  });
}

/**
 * An in-app modal dialog: role dialog, aria-modal, focus moved inside on open and trapped there,
 * the rest of the page inert except the live regions. Esc and Tab are handled on the whole host,
 * so they work wherever focus sits inside it; an Esc that ends an IME composition is the input
 * method's, not the dialog's. A press on the scrim leaves focus where it was; with `closeOnScrim`
 * (a dialog with nothing to lose: the palette, the Board's card) a click there closes it as Esc does.
 * Esc calls onClose, which may decline (a decision being sent). Focus starts on the first field
 * marked `data-autofocus`, else the first stop; with `focusTitle`, on the heading itself (a dialog
 * that is read first), whence Tab goes to the first stop after it in the DOM and Shift+Tab to the
 * last. A stop inside a collapsed (`hidden`) region is no stop. Dialogs stack: each makes what lies under it inert, the one under too, and
 * gives it back when it closes. With `top` (the document modal, docs/features/doc-modal.md
 * "Acceptance criteria", DM-10) it stays over a dialog of the view mounted in the same commit (a
 * reload of a card's modal with a document over it): what that one made inert of it as it mounted is
 * given back first. The caller returns focus on close. With `frame`, the caller lays the heading out
 * among its own parts (the work item's breadcrumbs above it, its columns below), the children after them.
 */
export function Dialog({
  title,
  onClose,
  children,
  className,
  closeOnScrim = false,
  focusTitle = false,
  top = false,
  frame,
}: {
  title: ReactNode;
  onClose: () => void;
  children?: ReactNode;
  className?: string;
  /** A click on the scrim calls onClose; off by default, so a misplaced click keeps typed text. */
  closeOnScrim?: boolean;
  /** Focus starts on the heading, which then takes focus by script alone. */
  focusTitle?: boolean;
  /** Over every dialog mounted with it: a sibling of the same commit that made this one inert gives it back. */
  top?: boolean;
  /** Lays the heading out with the caller's parts around it; the dialog's own `h2` all the same. */
  frame?: (heading: ReactNode) => ReactNode;
}) {
  const titleId = useId();
  const panel = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const root = panel.current;
    if (root === null) {
      return;
    }
    const host = root.closest<HTMLElement>("[data-dialog-host]");
    if (top && host !== null) {
      host.inert = false;
    }
    const preferred = focusTitle ? root.querySelector<HTMLElement>(".dialog-title") : root.querySelector<HTMLElement>("[data-autofocus]");
    (preferred ?? focusables(root)[0] ?? root).focus();
    const siblings = Array.from(document.body.children).filter(
      (element): element is HTMLElement =>
        element instanceof HTMLElement &&
        element !== host &&
        !element.inert &&
        !element.hasAttribute(LIVE_REGION_ATTRIBUTE),
    );
    for (const element of siblings) {
      element.inert = true;
    }
    return () => {
      for (const element of siblings) {
        element.inert = false;
      }
    };
  }, [focusTitle, top]);

  function onKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.key === "Escape") {
      if (event.nativeEvent.isComposing) {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      onClose();
      return;
    }
    const root = panel.current;
    if (event.key !== "Tab" || root === null) {
      return;
    }
    const items = focusables(root);
    const first = items[0];
    const last = items[items.length - 1];
    if (first === undefined || last === undefined) {
      event.preventDefault();
      return;
    }
    const active = document.activeElement;
    // A heading focused by script (`focusTitle`): Tab goes on to the first stop after it in the DOM
    // (the work item's breadcrumbs stand above it), Shift+Tab round to the last.
    if (active instanceof HTMLElement && active.classList.contains("dialog-title") && root.contains(active)) {
      event.preventDefault();
      if (event.shiftKey) {
        last.focus();
      } else {
        (items.find((item) => (active.compareDocumentPosition(item) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0) ?? first).focus();
      }
      return;
    }
    // The panel itself (focused by a click on its text) counts as outside: the browser's next stop
    // back from it lies before the dialog, so Tab goes to the first stop and Shift+Tab to the last.
    const inside = active !== root && root.contains(active);
    if (event.shiftKey && (active === first || !inside)) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && (active === last || !inside)) {
      event.preventDefault();
      first.focus();
    }
  }

  const heading = (
    <h2 id={titleId} className="dialog-title" tabIndex={focusTitle ? -1 : undefined}>
      {title}
    </h2>
  );

  return createPortal(
    <div className="dialog-host" data-dialog-host="" onKeyDown={onKeyDown}>
      <div
        className="dialog-scrim"
        data-dialog-scrim=""
        onMouseDown={(event) => {
          event.preventDefault();
        }}
        onClick={() => {
          if (closeOnScrim) {
            onClose();
          }
        }}
      />
      <div
        ref={panel}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
        className={className === undefined ? "dialog" : `dialog ${className}`}
      >
        {frame === undefined ? heading : frame(heading)}
        {children}
      </div>
    </div>,
    document.body,
  );
}
