// The bootstrap: the only place that picks the SpecEngineClient implementation and imports the
// mocks (ui/README.md "Contract seam"). The daemon by default
// (`docs/features/daemon-read.md` "Data"); `?scenario=<name>` before the `#` serves the mock
// instead, "Mock data" shown on every route. The mock client and its invented projects load in a
// chunk of their own, only under `?scenario=`: the daemon's page never fetches them. The catalog's
// instance starts with the first import in English; a page starting in Russian renders nothing until
// Russian's chunk lands or the wait runs out (docs/canon/ui-languages.md "Catalog"), so a local
// daemon's page never flashes English. The viewer's theme is applied before the root is made, so the
// first render is already in it (docs/features/design-foundation.md "Themes"); Inter's faces come
// before the tokens that name it. A lazy chunk gone after a redeploy reloads the page once per build,
// installed before the language's chunk is asked for (docs/features/canvas-clicks.md
// "Acceptance criteria", CK7).
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import type { SpecEngineClient } from "./api/client";
import { HttpClient } from "./api/http";
import { App } from "./app/App";
import { installReload } from "./app/reload";
import { applyTheme, storedTheme } from "./app/theme";
import { readyLanguage, useTranslate } from "./i18n";
import { fill, slot } from "./i18n/fill";
import { scenarioFromSearch } from "./mocks/scenario";
// React Flow's structural styles only (base.css, never style.css); its colours come from tokens.css.
import "@xyflow/react/dist/base.css";
import "./styles/fonts.css";
import "./styles/tokens.css";
import "./styles/app.css";

applyTheme(storedTheme());

const search = window.location.search;
const mocked = new URLSearchParams(search).has("scenario");
const scenario = scenarioFromSearch(search);
const container = document.getElementById("root");
if (container === null) {
  throw new Error("index.html lacks the root element");
}

/** The page's React root; a test unmounts it. */
export const root = createRoot(container);

function start(client: SpecEngineClient) {
  root.render(
    <StrictMode>
      <App client={client} scenario={mocked && scenario !== "normal" ? scenario : null} />
    </StrictMode>,
  );
}

/** Why the mock could not start, its chunk or its client failing: said in the page, never a blank one. */
function MockFailure({ error }: { error: unknown }) {
  const t = useTranslate();
  const said = error instanceof Error ? error.message : String(error);
  return <p role="alert">{fill(t("app.mock.failed", { error: slot("error") }), { error: <span lang="en">{said}</span> })}</p>;
}

function MockLoading() {
  const t = useTranslate();
  return (
    <p role="status" aria-busy="true">
      {t("app.mock.loading")}
    </p>
  );
}

function showFailure(error: unknown) {
  root.render(<MockFailure error={error} />);
}

// A plain dynamic import, never an await: awaited at the top, it would make the bundler pull
// every module the mock shares with the app out of the entry chunk (the entry may not be imported
// back while it waits), splitting the daemon's page into a score of small chunks. The language's
// gate is chained the same way, and never rejects.
function boot() {
  if (mocked) {
    root.render(<MockLoading />);
    import("./mocks/MockClient")
      .then(({ MockClient }) => {
        start(new MockClient(scenario));
      })
      .catch(showFailure);
  } else {
    start(new HttpClient());
  }
}

installReload(window, import.meta.url, {
  storage: () => window.sessionStorage,
  reload: () => {
    window.location.reload();
  },
});
void readyLanguage().then(boot);
