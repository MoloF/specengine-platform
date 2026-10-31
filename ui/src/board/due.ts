import { useNowOf } from "../ui/useNow";

// The Board's one clock (`docs/features/board-ui.md` "Board"): reads carry none, so the viewer's own
// calendar day is taken here and nowhere else in src/board/.

function pad(value: number, width: number): string {
  return String(value).padStart(width, "0");
}

/** The viewer's calendar day at `now`, `YYYY-MM-DD`, by the local date: a due date is a day, not an instant. */
export function localToday(now: number): string {
  const at = new Date(now);
  return `${pad(at.getFullYear(), 4)}-${pad(at.getMonth() + 1, 2)}-${pad(at.getDate(), 2)}`;
}

/** Today by the shared clock: it turns over within a tick of midnight, and no other tick re-renders. */
export function useToday(): string {
  return useNowOf(localToday);
}

/** `due < today` as strings, unless the card is closed: accepted or cancelled is never overdue. */
export function isOverdue({ due, accepted, cancelled }: { due: string | null; accepted: boolean; cancelled: boolean }, today: string): boolean {
  return due !== null && !accepted && !cancelled && due < today;
}
