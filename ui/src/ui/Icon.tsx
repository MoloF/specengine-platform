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
