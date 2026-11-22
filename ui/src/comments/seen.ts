import { useSyncExternalStore } from "react";

// What the Mentions view last showed (docs/features/comments.md "UI", Mentions): per project and
// login, the `id@version` of the rows of its first page, at most MENTIONS_SEEN_MAX, in `localStorage`
// as `specengine.mentions.seen.<p>[.<login>]`. The server keeps no reader state. Every storage access
// may throw (a private window, a policy): then nothing is kept, and the badge counts every row as
// unseen until the view shows them on this page, held here meanwhile.

/** The most rows kept: a first page's (`COMMENT_PAGE_MAX`). */
export const MENTIONS_SEEN_MAX = 100;

/** Where the rows last shown are kept: per login, browser-wide for a project when none (`login` null). */
export function mentionsSeenKey(project: string, login: string | null): string {
  return login === null ? `specengine.mentions.seen.${project}` : `specengine.mentions.seen.${project}.${login}`;
}

const listeners = new Set<() => void>();

/** What the browser refused to keep, by key: this page holds it instead. */
const unstored = new Map<string, string>();

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** The stored words as stored text, a stable snapshot: null when none, unreadable or refused. */
function storedText(key: string): string | null {
  const held = unstored.get(key);
  if (held !== undefined) {
    return held;
  }
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

/** The kept words as a set: none when nothing is kept or what is kept is not a list of words. */
export function seenOf(text: string | null): ReadonlySet<string> {
  if (text === null) {
    return new Set();
  }
  try {
    const value: unknown = JSON.parse(text);
    return Array.isArray(value) ? new Set(value.filter((word): word is string => typeof word === "string").slice(0, MENTIONS_SEEN_MAX)) : new Set();
  } catch {
    return new Set();
  }
}

/** Keeps the words of the rows shown, at most MENTIONS_SEEN_MAX; storage that refuses keeps them for this page. */
export function rememberSeen(project: string, login: string | null, words: readonly string[]): void {
  const key = mentionsSeenKey(project, login);
  const text = JSON.stringify(words.slice(0, MENTIONS_SEEN_MAX));
  if (storedText(key) === text) {
    return;
  }
  try {
    window.localStorage.setItem(key, text);
    unstored.delete(key);
  } catch {
    unstored.set(key, text);
  }
  for (const listener of listeners) {
    listener();
  }
}

/** The kept words' text for a project and login, following every keep on this page. */
export function useSeenText(project: string, login: string | null): string | null {
  const key = mentionsSeenKey(project, login);
  return useSyncExternalStore(subscribe, () => storedText(key));
}
