import { useSyncExternalStore } from "react";

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
