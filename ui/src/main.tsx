// The bootstrap: the only place that picks the SpecEngineClient implementation and imports the
// mocks (ui/README.md "Contract seam"). The daemon by default
// (`docs/features/daemon-read.md` "Data"); `?scenario=<name>` before the `#` serves the mock
// instead, "Mock data" shown on every route. The mock client and its invented projects load in a
// chunk of their own, only under `?scenario=`: the daemon's page never fetches them.
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import type { SpecEngineClient } from "./api/client";
import { HttpClient } from "./api/http";
import { App } from "./app/App";
import { scenarioFromSearch } from "./mocks/scenario";
// React Flow's structural styles only (base.css, never style.css); its colours come from tokens.css.
import "@xyflow/react/dist/base.css";
import "./styles/tokens.css";
import "./styles/app.css";

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
function showFailure(error: unknown) {
  root.render(
    <p role="alert">
      The mock data could not be loaded: {error instanceof Error ? error.message : String(error)}. Reload the page to try again.
    </p>,
  );
}

// A plain dynamic import, never an await: awaited at the top, it would make the bundler pull
// every module the mock shares with the app out of the entry chunk (the entry may not be imported
// back while it waits), splitting the daemon's page into a score of small chunks.
if (mocked) {
  root.render(
    <p role="status" aria-busy="true">
      Loading the mock data
    </p>,
  );
  import("./mocks/MockClient")
    .then(({ MockClient }) => {
      start(new MockClient(scenario));
    })
    .catch(showFailure);
} else {
  start(new HttpClient());
}
