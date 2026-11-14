import type { ApiError } from "./types";

// The hub's login door on a step-up route
// (docs/features/door-step-up.md "Description and interactions"; its "Data" the observed answer).
// The daemon never answers 401 (`docs/canon/hub.md` "Identity"), so on a hub every 401 is the
// door's: a one-factor session's `fetch` there gets a 401 whose body is one anchor to the door's
// login page. Nothing of the body is ever shown or parsed as markup: a regular expression takes its
// first `href`, of which only the origin is kept.

/** A refusal on a step-up surface: the door's, with its login page when one could be built, or no answer at all. */
export type DoorRefusal = { kind: "door"; link: string | null } | { kind: "noAnswer" };

/** The first double-quoted `href` attribute inside an `<a` tag: never the tail of another name, `data-href`. */
const FIRST_HREF = /<a\s[^>]*?(?<![\w-])href="([^"]*)"/i;

/**
 * The door's login page from the body's first `href`: its origin, served at the host's root, then
 * `/?rd=` and `here` percent-encoded, so the second factor lands on this page, not on the API URL the
 * body names. Null for no `href`, a relative one, any protocol but `https:`, or one carrying credentials.
 */
export function doorLink(body: string, here: string): string | null {
  const raw = FIRST_HREF.exec(body)?.[1];
  if (raw === undefined) {
    return null;
  }
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    return null;
  }
  if (url.protocol !== "https:" || url.username !== "" || url.password !== "") {
    return null;
  }
  return `${url.origin}/?rd=${encodeURIComponent(here)}`;
}

/** 401: the door's, `here` being location.href when the refusal came; 0: no answer; anything else: null, the daemon's words. */
export function doorRefusalOf(error: ApiError, here: string): DoorRefusal | null {
  if (error.status === 401) {
    return { kind: "door", link: doorLink(error.message, here) };
  }
  return error.status === 0 ? { kind: "noAnswer" } : null;
}
