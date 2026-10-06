import { useId, type ReactNode, type Ref } from "react";
import type { CheckFinding } from "../api/types";
import { RegionBoundary } from "../app/RegionBoundary";
import { sectionHash } from "../app/routes";
import { Icon } from "../ui/Icon";
import { isSpecPath } from "./findings";

/**
 * One region of Health: a `section` with its `h2`, an optional action beside the heading, its own
 * error boundary (a region that fails to render leaves the others standing) and `aria-busy` while
 * its read is being read.
 */
export function HealthRegion({
  title,
  boundary,
  headingRef,
  busy,
  action = null,
  className = "",
  children,
}: {
  title: string;
  /** The region's name in its boundary's message ("The <name> could not be shown"). */
  boundary: string;
  headingRef: Ref<HTMLHeadingElement>;
  busy: boolean;
  action?: ReactNode;
  className?: string;
  children: ReactNode;
}) {
  const id = useId();
  return (
    <section className={`health-region ${className}`.trim()} aria-labelledby={id} aria-busy={busy}>
      <header className="health-region-head">
        <h2 id={id} ref={headingRef} tabIndex={-1}>
          {title}
        </h2>
        {action}
      </header>
      <RegionBoundary name={boundary}>{children}</RegionBoundary>
    </section>
  );
}

/**
 * Where a finding is: `path:line`; an `.md` path links its node in the spec tree (no Tab stop of
 * its own: the row holds it, Enter follows it), any other path is text, `""` no file.
 */
export function FindingPlace({ project, finding, linked }: { project: string; finding: CheckFinding; linked: boolean }) {
  if (finding.path === "") {
    return <span className="finding-place muted">No file</span>;
  }
  const text = `${finding.path}:${String(finding.line)}`;
  if (linked && isSpecPath(finding.path)) {
    return (
      <a className="finding-place mono" href={sectionHash(project, "tree", finding.path)} tabIndex={-1}>
        {text}
      </a>
    );
  }
  return <span className="finding-place mono">{text}</span>;
}

/** A finding's subject as written, apart from its spoken label so its text is exactly the JSON's. */
export function FindingSubject({ subject }: { subject: string }) {
  if (subject === "") {
    return null;
  }
  return (
    <span className="finding-subject">
      <span className="sr-only">Subject: </span>
      <span className="finding-subject-text mono verbatim">{subject}</span>
    </span>
  );
}

/** A matched baseline entry: "until <expires>: <reason>", or expired, from `debt.expired` alone. */
export function DebtNote({ debt }: { debt: NonNullable<CheckFinding["debt"]> }) {
  return (
    <p className={debt.expired ? "finding-debt finding-debt-expired" : "finding-debt"}>
      <Icon name={debt.expired ? "debtExpired" : "debt"} />
      <span>
        {debt.expired ? "Debt expired " : "In debt until "}
        <span className="mono">{debt.expires}</span>: <span className="finding-debt-reason verbatim">{debt.reason}</span>
      </span>
    </p>
  );
}

/** A finding's message, exactly as sent: whitespace kept, never parsed or rendered as markup. */
export function FindingMessage({ message }: { message: string }) {
  return <p className="finding-message verbatim">{message}</p>;
}

/**
 * A count with its noun, read aloud whole ("ref-dangling, 3 findings"): the spaces are text of
 * their own between the parts, so a name computed from them keeps its words apart.
 */
export function Counted({ count, one, many }: { count: number; one: string; many: string }) {
  return (
    <>
      <span className="sr-only">,</span> <span className="health-count">{count}</span>{" "}
      <span className="sr-only">{count === 1 ? one : many}</span>
    </>
  );
}
