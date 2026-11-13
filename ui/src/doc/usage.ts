import type { ShownLink, ShownLinks } from "../api/types";
import { isResolvedLink, isWeakLink } from "../tree/labels";
import { kindSets, type InsideDirection, type KindSet } from "./inside";

// Who uses a document, laid out from the In rows its links read already serves
// (docs/features/usage-orphans.md "Acceptance criteria", UO10): the strong resolved rows gathered by
// their source, the other end's name, each source once with every place it cites from; the sources
// in other files grouped by direction, then raw kind, as Inside groups children; the holder's own
// file's apart. Nothing here counts a use: the count drawn is the daemon's `usage`, and the groups
// only lay out the rows it counted (docs/canon/architecture.md "Interface").

/** A group's users shown before "Show all". */
export const USED_BY_SHOWN = 12;

/** Where a source cites the node: the file and line the link is written at, its raw type (ADR-0031), and the section it lands on. */
export interface UsagePlace {
  path: string;
  line: number;
  type: string;
  at: string | null;
}

/** One source of In rows: its name (ID, else path), title, raw kind and every place it cites from, by path and line. */
export interface UsageUser {
  name: string;
  title: string | null;
  kind: string | null;
  places: UsagePlace[];
}

/** The users in other files, as sets; then those written in the holder's own file, outside the count. */
export interface UsageLayout {
  sets: KindSet<UsageUser>[];
  within: UsageUser[];
}

/** The strong In rows that resolve, each to a source, in served order of first citation; a source's places in file order. */
function usersIn(rows: readonly ShownLink[]): UsageUser[] {
  const users = new Map<string, UsageUser>();
  for (const row of rows) {
    if (!isResolvedLink(row) || isWeakLink(row)) {
      continue;
    }
    const place: UsagePlace = { path: row.path, line: row.line, type: row.type, at: row.at };
    const user = users.get(row.name);
    if (user === undefined) {
      users.set(row.name, { name: row.name, title: row.title, kind: row.end_kind, places: [place] });
    } else {
      user.places.push(place);
    }
  }
  for (const user of users.values()) {
    user.places.sort((a, b) => (a.path === b.path ? a.line - b.line : a.path < b.path ? -1 : 1));
  }
  return [...users.values()];
}

/**
 * The users of a holder at `path`: those whose rows are written in another file grouped by
 * direction, then kind (`kindSets`); those written in the holder's own file apart. Each set's
 * entries are distinct sources, so its size is how many sources it holds, never its rows.
 */
export function usageLayout(links: ShownLinks, path: string, directions: readonly InsideDirection[]): UsageLayout {
  const users = usersIn(links.incoming);
  const own = (user: UsageUser) => user.places.every((place) => place.path === path);
  return { sets: kindSets(users.filter((user) => !own(user)), directions), within: users.filter(own) };
}
