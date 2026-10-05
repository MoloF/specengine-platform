const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** How long ago a stored UTC time was, e.g. `12 min`; the raw text when it does not parse. */
export function formatAge(stored: string, now: number): string {
  const at = Date.parse(stored);
  if (Number.isNaN(at)) {
    return stored;
  }
  const elapsed = Math.max(0, now - at);
  if (elapsed < MINUTE) {
    return "just now";
  }
  if (elapsed < HOUR) {
    return `${Math.floor(elapsed / MINUTE)} min ago`;
  }
  if (elapsed < DAY) {
    return `${Math.floor(elapsed / HOUR)} h ago`;
  }
  return `${Math.floor(elapsed / DAY)} d ago`;
}

/** A stored time in UTC, minutes precision, e.g. `2026-10-05 21:14 UTC`; raw when it does not parse. */
export function formatUtc(stored: string): string {
  const at = Date.parse(stored);
  if (Number.isNaN(at)) {
    return stored;
  }
  return `${new Date(at).toISOString().slice(0, 16).replace("T", " ")} UTC`;
}
