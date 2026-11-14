// The hub's login door as a test sees it (docs/features/door-step-up.md "Data"): a one-factor
// session's `fetch` of a step-up route answered 401, `text/html`, the body one anchor to the door's
// login page carrying the refused URL (`rd`) and its method (`rm`), `&` written `&amp;`. The hosts are
// placeholders.

/** The door's body for a refused `path` of the hub, asked with `method`: the observed shape, on one line. */
export function doorBody(path: string, method = "GET"): string {
  return `<a href="https://login.example.org/?rd=${encodeURIComponent(`https://hub.example.org${path}`)}&amp;rm=${method}">401 Unauthorized</a>`;
}

/** The door's 401 as `fetch` meets it. */
export function doorAnswer(body: string): Response {
  return new Response(body, { status: 401, statusText: "Unauthorized", headers: { "Content-Type": "text/html; charset=utf-8" } });
}

/** The page the tests stand on when a refusal comes: a hub's origin, a placeholder. */
export const HUB_ORIGIN = "https://hub.example.org/";

/** What vitest's jsdom exposes to move the page's URL across origins; a test puts it back after. */
interface Reconfigurable {
  jsdom: { reconfigure(settings: { url: string }): void };
}

/** Moves the page to `url`, origin included, as a person's address bar would show it; no event fires. */
export function standAt(url: string): void {
  (globalThis as unknown as Reconfigurable).jsdom.reconfigure({ url });
}

/** jsdom's own page, which every other test stands on. */
export const TEST_ORIGIN = "http://localhost:3000/";
