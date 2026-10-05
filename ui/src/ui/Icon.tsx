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
