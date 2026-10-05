import { useSyncExternalStore, type MouseEvent } from "react";

// The hash is the route. Section and project changes push a history entry (links, pushHash);
// a selection inside a view replaces the current one (replaceHash).

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

export function replaceHash(hash: string): void {
  window.history.replaceState(window.history.state, "", hash);
  notify();
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
