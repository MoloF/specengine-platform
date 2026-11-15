// "New since your last visit" (docs/features/auto-feed.md "The UI"): per viewer, the highest `seq`
// the Activity view showed, `specengine.feed.seen.<p>`, on a hub `specengine.feed.seen.<p>.<login>`
// (docs/canon/ui-client.md "Stored choices"). Read when the view opens, written when it closes and
// on Mark all seen, never on open. Every storage access may throw (a private window, a policy): then
// nothing is kept and the view still shows what it reads, its marks held in its own state.

/** Where the last seen `seq` is kept: per login on a hub, browser-wide locally (`login` null). */
export function seenKey(project: string, login: string | null): string {
  return login === null ? `specengine.feed.seen.${project}` : `specengine.feed.seen.${project}.${login}`;
}

/** The stored `seq`, a non-negative whole number; null when none, unreadable or refused. */
export function recallSeen(project: string, login: string | null): number | null {
  try {
    const stored = window.localStorage.getItem(seenKey(project, login));
    return stored !== null && /^\d{1,15}$/.test(stored) ? Number(stored) : null;
  } catch {
    return null;
  }
}

/** Keeps `seq` as seen; storage that refuses keeps nothing, and the next visit marks as this one did. */
export function rememberSeen(project: string, login: string | null, seq: number): void {
  try {
    window.localStorage.setItem(seenKey(project, login), String(seq));
  } catch {
    // Nothing kept: the page stays correct, its marks from what it read.
  }
}
