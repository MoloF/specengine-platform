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

/** The dialog's tab stops in order; a radio group is one stop, its checked radio (else its first). */
function focusables(root: HTMLElement): HTMLElement[] {
  const radios = Array.from(root.querySelectorAll<HTMLInputElement>('input[type="radio"]'));
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE)).filter((element) => {
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
 * (a dialog with nothing to lose, the palette) a click there closes it as Esc does. Esc calls
 * onClose, which may decline (a decision being sent). The caller returns focus on close.
 */
export function Dialog({
  title,
  onClose,
  children,
  className,
  closeOnScrim = false,
}: {
  title: ReactNode;
  onClose: () => void;
  children: ReactNode;
  className?: string;
  /** A click on the scrim calls onClose; off by default, so a misplaced click keeps typed text. */
  closeOnScrim?: boolean;
}) {
  const titleId = useId();
  const panel = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const root = panel.current;
    if (root === null) {
      return;
    }
    const preferred = root.querySelector<HTMLElement>("[data-autofocus]");
    (preferred ?? focusables(root)[0] ?? root).focus();
    const host = root.closest<HTMLElement>("[data-dialog-host]");
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
  }, []);

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
        <h2 id={titleId} className="dialog-title">
          {title}
        </h2>
        {children}
      </div>
    </div>,
    document.body,
  );
}
