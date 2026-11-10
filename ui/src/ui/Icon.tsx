// Hand-drawn 16 px icons (no icon package). Decorative: every use sits beside a text label.

const PATHS = {
  severityHigh: "M8 2.2 14.3 13.5H1.7L8 2.2Z M8 6.4v3.4 M8 11.6v.2",
  severityNormal: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11Z M5.5 8h5",
  severityLow: "M3.5 6 8 10.5 12.5 6",
  unknown: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11Z M6.4 6.4a1.7 1.7 0 1 1 2.4 1.6c-.5.3-.8.6-.8 1.2 M8 11.2v.2",
  open: "M8 3a5 5 0 1 0 0 10A5 5 0 0 0 8 3Z",
  changes: "M6 4 2.5 7.5 6 11 M2.5 7.5H10a3.5 3.5 0 0 1 0 7h-1",
  approved: "M3 8.5 6.5 12 13 4.5",
  applied: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11Z M5.3 8.2l1.9 1.9 3.6-4",
  rejected: "M4 4l8 8 M12 4l-8 8",
  deferred: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11Z M8 5v3.3l2.2 1.4",
  superseded: "M2.5 8h10 M9 4.5 12.5 8 9 11.5",
  mock: "M6 2h4 M6.8 2v4L3 13a.8.8 0 0 0 .7 1.2h8.6A.8.8 0 0 0 13 13L9.2 6V2 M4.6 10h6.8",
  keyboard: "M1.8 4.5h12.4v7H1.8z M4 7h.1 M6.5 7h.1 M9 7h.1 M11.5 7h.1 M5 9.3h6",
  alert: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11Z M8 5v3.6 M8 10.8v.2",
  retry: "M13 8a5 5 0 1 1-1.5-3.6 M13 2.8v2.6h-2.6",
  recommended: "M8 2.3l1.7 3.6 3.9.5-2.9 2.7.8 3.9L8 11.1 4.5 13l.8-3.9-2.9-2.7 3.9-.5L8 2.3Z",
  info: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11Z M8 7.2v3.8 M8 5v.2",
  chevronRight: "M6 3.5 10.5 8 6 12.5",
  chevronDown: "M3.5 6 8 10.5 12.5 6",
  linkResolved: "M6.5 9.5l3-3 M7 4.5l1-1a2.5 2.5 0 0 1 3.5 3.5l-1 1 M9 11.5l-1 1a2.5 2.5 0 0 1-3.5-3.5l1-1",
  linkDangling: "M7 4.5l1-1a2.5 2.5 0 0 1 3.5 3.5l-1 1 M9 11.5l-1 1a2.5 2.5 0 0 1-3.5-3.5l1-1 M2.5 2.5l2 2 M13.5 13.5l-2-2",
  linkSkipped: "M9 2.5h4.5V7 M13.5 2.5 7.5 8.5 M11.5 9.5v4h-9v-9h4",
  linkUnchecked: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11Z M4.2 11.8l7.6-7.6",
  parentDangling: "M8 13.5V6.5 M5 9.5l3-3 3 3 M2.5 2.5h4 M9.5 2.5h4",
  parentCycle: "M12.5 6.5A4.6 4.6 0 0 0 4 5.2 M4 2.4v2.8h2.8 M3.5 9.5a4.6 4.6 0 0 0 8.5 1.3 M12 13.6v-2.8H9.2",
  archived: "M2.5 3h11v3.2h-11z M3.5 6.2v6.8h9V6.2 M6.5 9h3",
  search: "M7 2.5a4.5 4.5 0 1 0 0 9 4.5 4.5 0 0 0 0-9Z M10.3 10.3l3.2 3.2",
  inbox: "M2.5 9h3l1 2h3l1-2h3 M2.5 9 4.5 3.5h7L13.5 9v4h-11Z",
  back: "M6.5 3.5 2.5 8l4 4.5 M2.5 8h11",
  arrowRight: "M2.5 8h10 M9 4.5 12.5 8 9 11.5",
  arrowLeft: "M13.5 8h-10 M7 4.5 3.5 8 7 11.5",
  focus: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11Z M8 6a2 2 0 1 0 0 4 2 2 0 0 0 0-4Z M8 1v2 M8 13v2 M1 8h2 M13 8h2",
  selected: "M2.5 8.5 6 12l7.5-8",
  stub: "M2.5 2.5h3 M10.5 2.5h3v3 M13.5 10.5v3h-3 M5.5 13.5h-3v-3 M2.5 5.5v0 M7 2.5h2 M7 13.5h2 M2.5 7v2 M13.5 7v2",
  more: "M3.5 8h.1 M8 8h.1 M12.5 8h.1",
  fit: "M2.5 6V2.5H6 M10 2.5h3.5V6 M13.5 10v3.5H10 M6 13.5H2.5V10 M6 8h4 M8 6v4",
  graph: "M2.5 4.5a1.5 1.5 0 1 0 3 0 1.5 1.5 0 1 0-3 0Z M10.5 3.5a1.5 1.5 0 1 0 3 0 1.5 1.5 0 1 0-3 0Z M10.5 12.5a1.5 1.5 0 1 0 3 0 1.5 1.5 0 1 0-3 0Z M5.5 4.3l5-.6 M5 5.6l5.9 5.8",
  taskDraft: "M10.5 2.5l3 3L6 13H3v-3l7.5-7.5Z M9 4l3 3",
  taskReview: "M1.5 8s2.4-4.5 6.5-4.5S14.5 8 14.5 8 12.1 12.5 8 12.5 1.5 8 1.5 8Z M8 6.2a1.8 1.8 0 1 0 0 3.6 1.8 1.8 0 0 0 0-3.6Z",
  taskReady: "M3.5 14V2.5 M3.5 3h8l-1.8 2.75 1.8 2.75h-8",
  taskInProgress: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11Z M6.7 5.6 10.4 8l-3.7 2.4Z",
  taskInReview: "M2.5 2.5h8v6 M2.5 2.5v11h5 M4.5 5.5h4 M4.5 8h2 M11 9.5a2.2 2.2 0 1 0 0 4.4 2.2 2.2 0 0 0 0-4.4Z M12.6 13.2l1.4 1.3",
  taskAccepted: "M1.5 8.5l3 3L10 5 M7.2 10.8l.7.7 6.6-7",
  taskCancelled: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11Z M6 6l4 4 M10 6l-4 4",
  runPartial: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11Z M8 2.5v11 M8 5.5h3.8 M8 8h5.5 M8 10.5h3.8",
  runAbandoned: "M9.5 3.5h-6v9h6 M7 8h7 M11.5 5.5 14 8l-2.5 2.5",
  specChanged: "M3 2.5h10v11H3Z M8 4.5v4 M6 6.5h4 M6 11h4",
  waiting: "M4.5 2.5h7 M4.5 13.5h7 M5.5 2.5V4L8 8l-2.5 4v1.5 M10.5 2.5V4L8 8l2.5 4v1.5",
  copy: "M5.5 5.5h8v8h-8Z M10.5 5.5v-3h-8v8h3",
  terminal: "M2 3h12v10H2Z M4.5 6.5l2 1.5-2 1.5 M8 10h3.5",
  taskItemDone: "M2.5 2.5h11v11h-11Z M5 8.2l2 2 4-4.4",
  taskItemOpen: "M2.5 2.5h11v11h-11Z",
  // spec check (docs/features/ui-health.md): a verdict each, two finding severities, debt.
  checkClean: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11Z M5.3 8.2l1.9 1.9 3.6-4",
  checkObserved: "M1.8 8.6 4.8 11.6 10.4 5 M13.2 3.5v4.4 M13.2 10.6v.2",
  checkFails: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11Z M6 6l4 4 M10 6l-4 4",
  cannotCheck: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11Z M4.2 11.8l7.6-7.6",
  findingError: "M8 1.8 14.2 8 8 14.2 1.8 8Z M8 5.2v3.4 M8 10.6v.2",
  findingWarning: "M8 2.2 14.3 13.5H1.7L8 2.2Z M8 6.4v3.4 M8 11.6v.2",
  debt: "M2.5 3.5h11v10h-11Z M2.5 6.5h11 M5.5 2v3 M10.5 2v3",
  // The Roadmap (docs/features/roadmap-screen.md): the current release, the MVP cut's rule.
  current: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11Z M8 5.8a2.2 2.2 0 1 0 0 4.4 2.2 2.2 0 0 0 0-4.4Z",
  cutLine: "M1.5 8h2.5 M6.75 8h2.5 M12 8h2.5 M8 2.5v3 M8 10.5v3",
  debtExpired: "M2.5 3.5h11v10h-11Z M2.5 6.5h11 M5.5 2v3 M10.5 2v3 M6 8.6l4 3.3 M10 8.6l-4 3.3",
  // The Flow (docs/features/flow-canvas.md): a start, an end, a gap, a box marked by a gap item, the arc.
  flowStart: "M4.5 3v10 M7.5 4.5 12 8l-4.5 3.5Z",
  flowEnd: "M3.5 3v10 M6.5 3.5h6.5v9H6.5Z",
  flowGap: "M2.5 5V2.5H5 M11 2.5h2.5V5 M13.5 11v2.5H11 M5 13.5H2.5V11 M8 5.2v3.4 M8 10.8v.2",
  flowMarked: "M8 2.5 9.7 6l3.8.5-2.8 2.6.7 3.8L8 11.1l-3.4 1.8.7-3.8L2.5 6.5 6.3 6 8 2.5Z",
  flowLoop: "M12.5 8a4.5 4.5 0 1 1-1.4-3.3 M12.8 2.6v2.6h-2.6",
  // The Board (docs/features/board-ui.md): a late card, moving one, a new one, and Up and Down.
  cardOverdue: "M7 2.6a5.2 5.2 0 1 0 5.4 5.4 M7 5v3l2 1.2 M13 2.5v3 M13 7.6v.2",
  cardMove: "M2.5 8h11 M5 5.5 2.5 8 5 10.5 M11 5.5 13.5 8 11 10.5",
  cardNew: "M8 3v10 M3 8h10",
  cardUp: "M8 13V3 M4.5 6.5 8 3l3.5 3.5",
  cardDown: "M8 3v10 M4.5 9.5 8 13l3.5-3.5",
  // Against the brief (docs/features/card-trace.md): the result went another way.
  cardDeparted: "M3 13 13 3 M7.5 3H13v5.5",
  // A hand-off and a return (docs/canon/card-flows.md "The rule"): work passed to another trade, work sent back.
  cardHandOff: "M2.5 3.5h5v9h-5Z M7.5 8h6 M11 5.5 13.5 8 11 10.5",
  cardReturned: "M13 4.5v3a3 3 0 0 1-3 3H3.5 M6 8 3.5 10.5 6 13",
  // A lane folded or open, and a card on a page of its own.
  laneClosed: "M6 3.5 10.5 8 6 12.5",
  laneOpen: "M3.5 6 8 10.5 12.5 6",
  cardPage: "M9 2.5h4.5V7 M13.5 2.5 7.5 8.5 M11.5 9.5v4h-9v-9h4",
  // The document view (docs/features/doc-modal.md "Acceptance criteria", DM-12): a page with its corner turned.
  document: "M3.5 1.5h6l3 3v10h-9Z M9.5 1.5v3h3 M5.5 8h5 M5.5 10.5h5",
  // The header (docs/features/design-foundation.md "The header"): the theme's three choices, the
  // language, the project and the workspace, each in front of its select's value.
  themeSystem: "M2 3h12v8H2Z M6 14h4 M8 11v3",
  themeLight: "M8 5.2a2.8 2.8 0 1 0 0 5.6 2.8 2.8 0 0 0 0-5.6Z M8 1.5v1.4 M8 13.1v1.4 M1.5 8h1.4 M13.1 8h1.4 M3.4 3.4l1 1 M11.6 11.6l1 1 M3.4 12.6l1-1 M11.6 4.4l1-1",
  themeDark: "M13.5 9.6A5.6 5.6 0 0 1 6.4 2.5a5.6 5.6 0 1 0 7.1 7.1Z",
  language: "M8 2.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11Z M2.5 8h11 M8 2.5c1.5 1.6 2.3 3.4 2.3 5.5S9.5 11.9 8 13.5 M8 2.5C6.5 4.1 5.7 5.9 5.7 8s.8 3.9 2.3 5.5",
  project: "M2 3.5h4l1.5 1.8H14v7.2H2Z",
  workspace: "M2.5 2.5h4.5v4.5H2.5Z M9 2.5h4.5v4.5H9Z M2.5 9h4.5v4.5H2.5Z M9 9h4.5v4.5H9Z",
} as const;

export type IconName = keyof typeof PATHS;

export function Icon({ name }: { name: IconName }) {
  return (
    <svg
      className="icon"
      viewBox="0 0 16 16"
      width="16"
      height="16"
      aria-hidden="true"
      focusable="false"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d={PATHS[name]} />
    </svg>
  );
}
