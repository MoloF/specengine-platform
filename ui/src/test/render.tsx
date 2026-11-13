import { fireEvent, render, screen, within } from "@testing-library/react";
import { App } from "../app/App";
import type { SpecEngineClient } from "../api/client";

/** Renders the whole UI at `hash` over `client`. */
export function renderApp(client: SpecEngineClient, hash: string, scenario: string | null = null) {
  window.history.replaceState(null, "", `/${hash}`);
  return render(<App client={client} scenario={scenario} />);
}

/**
 * Picks the Graph home's Rings view: the Map is the home's first view since corpus-map
 * (docs/features/corpus-map.md "Acceptance criteria", CM8), so a test of the rings asks for them.
 */
export async function showRings(): Promise<void> {
  const view = await screen.findByRole("group", { name: "View" });
  fireEvent.click(within(view).getByRole("button", { name: "Rings" }));
}
