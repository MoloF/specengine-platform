import type { CatalogDocument } from "../api/types";

// The direction's table on the stub client
// (docs/features/direction-catalog.md "Acceptance criteria", DC-9, DC-10): the spec's scenario as
// its JSON pins it, a git copy of
// `fixtures/studio-lamp` with Ann Owner applying PR-0001 to the arrival and Bob Hand editing the storm
// by hand; its OIDs, which the spec gives by shape, here 40 hex digits each. The usage
// (docs/features/usage-orphans.md "Acceptance criteria", UO5): the storm follows the arrival, so the
// arrival is used by one and the storm, linking out, is no orphan.

/** The spec's JSON, verbatim but the OIDs. */
export function pinnedCatalog(): CatalogDocument {
  return {
    kinds: ["scene"],
    archive: false,
    notes: [],
    rows: [
      {
        id: "SCN-ARRIVAL",
        kind: "scene",
        title: "Arrival",
        path: "docs/spec/scn-arrival.md",
        line: 1,
        rev: 1,
        status: null,
        archived: false,
        state: "in work",
        open_items: 1,
        tasks: 1,
        tasks_open: 1,
        used_by: 1,
        mentioned_by: 0,
        orphan: false,
        last_change: { at: "2026-01-02T10:00:00Z", by: "Ann Owner <ann@example.org>", by_name: "Ann Owner", commit: "0a1b2c3d4e5f60718293a4b5c6d7e8f901234567", proposal: "PR-0001", of_file: false },
      },
      {
        id: "SCN-STORM",
        kind: "scene",
        title: "The storm",
        path: "docs/spec/scn-storm.md",
        line: 1,
        rev: 1,
        status: null,
        archived: false,
        state: "no card",
        open_items: 0,
        tasks: 0,
        tasks_open: 0,
        used_by: 0,
        mentioned_by: 0,
        orphan: false,
        last_change: { at: "2026-01-03T10:00:00Z", by: "Bob Hand <bob@example.org>", by_name: "Bob Hand", commit: "fedcba9876543210fedcba9876543210fedcba98", proposal: null, of_file: false },
      },
    ],
    truncated: false,
    omitted: null,
  };
}
