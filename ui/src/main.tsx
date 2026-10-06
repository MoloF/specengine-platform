// The bootstrap: the only place that picks the SpecEngineClient implementation and imports the
// mocks (ui/README.md "Contract seam"). The daemon by default
// (`docs/features/daemon-read.md` "Data"); `?scenario=<name>` before the `#` serves the mock
// instead, "Mock data" shown on every route.
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import type { SpecEngineClient } from "./api/client";
import { HttpClient } from "./api/http";
import { App } from "./app/App";
import { MockClient } from "./mocks/MockClient";
import { scenarioFromSearch } from "./mocks/scenario";
// React Flow's structural styles only (base.css, never style.css); its colours come from tokens.css.
import "@xyflow/react/dist/base.css";
import "./styles/tokens.css";
import "./styles/app.css";

const search = window.location.search;
const mocked = new URLSearchParams(search).has("scenario");
const scenario = scenarioFromSearch(search);
const client: SpecEngineClient = mocked ? new MockClient(scenario) : new HttpClient();
const container = document.getElementById("root");
if (container === null) {
  throw new Error("index.html lacks the root element");
}

/** The page's React root; a test unmounts it. */
export const root = createRoot(container);
root.render(
  <StrictMode>
    <App client={client} scenario={mocked && scenario !== "normal" ? scenario : null} />
  </StrictMode>,
);
