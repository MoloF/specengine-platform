import { render } from "@testing-library/react";
import { App } from "../app/App";
import type { SpecEngineClient } from "../api/client";

/** Renders the whole UI at `hash` over `client`. */
export function renderApp(client: SpecEngineClient, hash: string, scenario: string | null = null) {
  window.history.replaceState(null, "", `/${hash}`);
  return render(<App client={client} scenario={scenario} />);
}
