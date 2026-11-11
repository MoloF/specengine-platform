// A lazy chunk gone after a redeploy (docs/features/canvas-clicks.md "Acceptance criteria", CK6, CK7):
// the page still names the old build's chunks, the server has the new build's, and `React.lazy`
// keeps the failed import until the page reloads, so Retry cannot recover. Vite's preload helper
// dispatches `vite:preloadError` on the window for such an import; the page then reloads once per
// build, onto the new one (every asset is served `no-store`). The mark in sessionStorage holds the
// build that already reloaded, so a deploy that is itself broken never loops: its route keeps its
// error and Retry. When the mark can be neither read nor written, nothing reloads. The event is never
// prevented: the import would then resolve to `undefined`, which `lazy` turns into another error.
// Silent: no word is shown.

/** The sessionStorage key: the build a failed chunk has already reloaded once. */
export const RELOAD_MARK = "specengine.reloaded";

export interface ReloadDeps {
  /** Called inside the try: reaching sessionStorage may throw too. */
  storage: () => Pick<Storage, "getItem" | "setItem">;
  reload: () => void;
}

/** One `vite:preloadError`: true when it reloaded. Never calls preventDefault. */
export function onPreloadError(build: string, deps: ReloadDeps): boolean {
  try {
    if (deps.storage().getItem(RELOAD_MARK) === build) {
      return false;
    }
  } catch {
    return false;
  }
  try {
    deps.storage().setItem(RELOAD_MARK, build);
  } catch {
    return false;
  }
  deps.reload();
  return true;
}

/** Adds the one `vite:preloadError` listener to `target`. */
export function installReload(target: Pick<Window, "addEventListener">, build: string, deps: ReloadDeps): void {
  target.addEventListener("vite:preloadError", () => {
    onPreloadError(build, deps);
  });
}
