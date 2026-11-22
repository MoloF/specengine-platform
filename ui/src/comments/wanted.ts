// The comment a Mentions row was followed to (docs/features/comments.md "UI", Mentions: "scrolled to
// the comment"): kept for the page until the thread holding it is drawn, which brings it into view
// and focuses it, then forgets it. The row's own navigation is the one route change it outlives: the
// next one forgets it, so a thread met later elsewhere never jumps. A link opened elsewhere (a new
// tab) lands on the target alone.

let wanted: { project: string; on: string; id: string; arrived: boolean } | null = null;

/** The route changes after a want: the first is the row's own; the next forgets it. */
function onRouteChange(): void {
  if (wanted !== null && !wanted.arrived) {
    wanted.arrived = true;
    return;
  }
  forgetWanted();
}

/** Asks the thread under `on` in `project` to bring the comment `id` into view. */
export function wantComment(project: string, on: string, id: string): void {
  forgetWanted();
  wanted = { project, on, id, arrived: false };
  window.addEventListener("hashchange", onRouteChange);
}

/** The wanted comment of the thread under `on` in `project`, if any: still wanted until `forgetWanted`. */
export function wantedIn(project: string, on: string): string | null {
  return wanted !== null && wanted.project === project && wanted.on === on ? wanted.id : null;
}

/** The wanted comment is drawn, or no longer sought. */
export function forgetWanted(): void {
  wanted = null;
  window.removeEventListener("hashchange", onRouteChange);
}
