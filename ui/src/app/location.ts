import { useSyncExternalStore, type MouseEvent } from "react";
import { docOf, withDoc, withoutDoc } from "./routes";

// The hash is the route. Section and project changes push a history entry (links, pushHash);
// a selection inside a view replaces the current one (replaceHash). The document modal is the hash's
// `doc` pair on any project route (docs/features/doc-modal.md "Acceptance criteria", DM-5, DM-6): a
// screen's own replace keeps it, a push drops it, and the modal's own close takes it away.

const listeners = new Set<() => void>();

function notify(): void {
  for (const listener of listeners) {
    listener();
  }
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  window.addEventListener("hashchange", listener);
  window.addEventListener("popstate", listener);
  return () => {
    listeners.delete(listener);
    window.removeEventListener("hashchange", listener);
    window.removeEventListener("popstate", listener);
  };
}

function snapshot(): string {
  return window.location.hash;
}

export function useHash(): string {
  return useSyncExternalStore(subscribe, snapshot);
}

export function pushHash(hash: string): void {
  window.location.hash = hash;
}

/**
 * Replaces the current entry. `keepDoc` (the default) carries an open modal's `doc` pair over to a
 * hash that names none, so a screen's own replace (a selection, a seek, a correction) never closes
 * it; the modal's own close and the landing pass `false`.
 */
export function replaceHash(hash: string, options: { keepDoc?: boolean } = {}): void {
  const open = options.keepDoc === false ? null : docOf(window.location.hash);
  const next = open !== null && docOf(hash) === null ? withDoc(hash, open) : hash;
  window.history.replaceState(window.history.state, "", next);
  notify();
}

/** The document modal's entry was pushed by this page: its close goes back. Forgotten once the modal is gone. */
let docPushed = false;

/** What opened the modal, a drawn sector among them: focus goes back to it on close while it stands and is not inert. */
let docOpener: HTMLElement | SVGElement | null = null;

/** The element focus stands on, when it stands on one: the opener of a modal opened by a key or a menu. */
function focused(): HTMLElement | null {
  const active = document.activeElement;
  return active instanceof HTMLElement && active !== document.body ? active : null;
}

/**
 * Opens the document modal on `ref` from `opener`: one pushed entry when none is open, the open one
 * replaced when one is, so a walk inside the modal is one entry however far it goes. `base` is the
 * route it opens over (the current one by default; the palette gives a project's home when the
 * current route shows no modal of that project). The entry counts as this page's to go back from only
 * when its route can show the modal.
 */
export function openDocFrom(ref: string, opener: HTMLElement | SVGElement | null, base: string = window.location.hash): void {
  const current = window.location.hash;
  if (base === current && docOf(current) !== null) {
    replaceHash(withDoc(current, ref));
    return;
  }
  const target = withDoc(base, ref);
  docOpener = opener;
  docPushed = docOf(target) !== null;
  pushHash(target);
}

/** Opens the document modal on `ref`, the focused element its opener: the palette, a key, an act. */
export function openDoc(ref: string): void {
  openDocFrom(ref, focused());
}

/** Esc, Close and the scrim: back when this page pushed the modal's entry, else the hash without `doc` in its place. */
export function closeDoc(): void {
  if (docPushed) {
    docPushed = false;
    window.history.back();
  } else {
    replaceHash(withoutDoc(window.location.hash), { keepDoc: false });
  }
}

/** The modal is gone (closed, Back, a push): its entry and opener are no longer this page's to go back to. */
export function forgetDoc(): HTMLElement | SVGElement | null {
  const opener = docOpener;
  docPushed = false;
  docOpener = null;
  return opener;
}

/**
 * Whether a click on an in-app anchor moves this page to another hash, as the browser follows it:
 * the primary button with no modifier (Cmd, Ctrl, Shift or Alt open it elsewhere) and a hash other
 * than the current one (the same hash moves nothing, no hashchange). Tells a followed link from one
 * that went nowhere here.
 */
export function movesHere(event: MouseEvent<HTMLAnchorElement>): boolean {
  return (
    event.button === 0 &&
    !event.defaultPrevented &&
    !event.altKey &&
    !event.ctrlKey &&
    !event.metaKey &&
    !event.shiftKey &&
    event.currentTarget.hash !== window.location.hash
  );
}
