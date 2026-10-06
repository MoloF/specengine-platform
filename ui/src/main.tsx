// The bootstrap: the only place that picks the SpecEngineClient implementation and imports the
// mocks (ui/README.md "Contract seam"). Switching to the daemon replaces these lines.
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App";
import { MockClient } from "./mocks/MockClient";
import { scenarioFromSearch } from "./mocks/scenario";
// React Flow's structural styles only (base.css, never style.css); its colours come from tokens.css.
import "@xyflow/react/dist/base.css";
import "./styles/tokens.css";
import "./styles/app.css";

const scenario = scenarioFromSearch(window.location.search);
const client = new MockClient(scenario);
const container = document.getElementById("root");
if (container === null) {
  throw new Error("index.html lacks the root element");
}
createRoot(container).render(
  <StrictMode>
    <App client={client} scenario={scenario === "normal" ? null : scenario} />
  </StrictMode>,
);
