import { useSyncExternalStore } from "react";

// A shared clock for ages, read through an external store so render stays pure.

const TICK_MS = 30_000;
const listeners = new Set<() => void>();
let now = Date.now();
let timer: ReturnType<typeof setInterval> | null = null;

function subscribe(listener: () => void): () => void {
  if (listeners.size === 0) {
    now = Date.now();
    timer = setInterval(() => {
      now = Date.now();
      for (const each of listeners) {
        each();
      }
    }, TICK_MS);
  }
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
    if (listeners.size === 0 && timer !== null) {
      clearInterval(timer);
      timer = null;
    }
  };
}

function snapshot(): number {
  return now;
}

export function useNow(): number {
  return useSyncExternalStore(subscribe, snapshot);
}
