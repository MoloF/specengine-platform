import { useId, type ReactNode, type Ref } from "react";
import type { CheckFinding } from "../api/types";
import { RegionBoundary } from "../app/RegionBoundary";
import { DocLink } from "../doc/DocLink";
import { useTranslate } from "../i18n";
import { around, fill, slot } from "../i18n/fill";
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
 * Where a finding is: `path:line`; an `.md` path links its document (no Tab stop of its own: the row
 * holds it, Enter opens it), any other path is text, `""` no file.
 */
export function FindingPlace({ project, finding, linked }: { project: string; finding: CheckFinding; linked: boolean }) {
  const t = useTranslate();
  if (finding.path === "") {
    return <span className="finding-place muted">{t("health.noFile")}</span>;
  }
  const text = `${finding.path}:${String(finding.line)}`;
  if (linked && isSpecPath(finding.path)) {
    return (
      <DocLink className="finding-place mono" project={project} docRef={finding.path} tabIndex={-1}>
        {text}
      </DocLink>
    );
  }
  return <span className="finding-place mono">{text}</span>;
}

/** A finding's subject as written, apart from its spoken label so its text is exactly the JSON's. */
export function FindingSubject({ subject }: { subject: string }) {
  const t = useTranslate();
  if (subject === "") {
    return null;
  }
  return (
    <span className="finding-subject">
      <span className="sr-only">{t("health.subject")}</span>
      <span className="finding-subject-text mono verbatim">{subject}</span>
    </span>
  );
}

/** A matched baseline entry: "until <expires>: <reason>", or expired, from `debt.expired` alone. */
export function DebtNote({ debt }: { debt: NonNullable<CheckFinding["debt"]> }) {
  const t = useTranslate();
  return (
    <p className={debt.expired ? "finding-debt finding-debt-expired" : "finding-debt"}>
      <Icon name={debt.expired ? "debtExpired" : "debt"} />
      <span>
        {fill(t(debt.expired ? "health.debtExpired" : "health.debtUntil", { expires: slot("expires"), reason: slot("reason") }), {
          expires: <span className="mono">{debt.expires}</span>,
          reason: <span className="finding-debt-reason verbatim">{debt.reason}</span>,
        })}
      </span>
    </p>
  );
}

/** A finding's message, exactly as sent: whitespace kept, never parsed or rendered as markup. */
export function FindingMessage({ message }: { message: string }) {
  return (
    <p className="finding-message verbatim" lang="en">
      {message}
    </p>
  );
}

/**
 * A count with its noun, read aloud whole ("ref-dangling, 3 findings"): `said` is the counted
 * phrase in the language shown, its number drawn, the words around it read out; the spaces are text
 * of their own between the parts, so a name computed from them keeps its words apart.
 */
export function Counted({ count, said }: { count: number; said: string }) {
  const [before, after] = around(said, String(count));
  return (
    <>
      <span className="sr-only">,</span> {before !== "" && <span className="sr-only">{before}</span>}
      <span className="health-count">{count}</span> <span className="sr-only">{after.trim()}</span>
    </>
  );
}
