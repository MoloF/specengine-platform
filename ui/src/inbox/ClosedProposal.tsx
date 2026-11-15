import { Fragment, useId } from "react";
import { useProjectName } from "../api/queries";
import type { Proposal } from "../api/types";
import { homeHash, sectionHash } from "../app/routes";
import { DocLink } from "../doc/DocLink";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { Breadcrumbs, type Crumb } from "../ui/Breadcrumbs";
import { Icon } from "../ui/Icon";
import { IssueView } from "../ui/IssueView";
import { Lozenge } from "../ui/Lozenge";
import { PropertyList, type Property } from "../ui/PropertyList";
import { Section } from "../ui/states";
import { shownSlot, shownText } from "../ui/text";
import { formatUtc } from "../ui/time";
import { statusLozenge } from "./labels";
import { targetsOf } from "./targets";

/** A text the author wrote, unless it is blank. */
function written(text: string | null): string | null {
  return text !== null && text.trim() !== "" ? text : null;
}

/**
 * Who decided, as stored (`Name <address>` or a login): a line may break after each `@` and `.`, so
 * a narrow column wraps the address at its parts and never leaves its closing `>` alone on a line.
 */
function Decider({ text }: { text: string }) {
  const parts = shownSlot(text).split(/(?<=[@.])/u);
  return (
    <bdi>
      {parts.map((part, index) => (
        <Fragment key={index}>
          {index > 0 && <wbr />}
          {part}
        </Fragment>
      ))}
    </bdi>
  );
}

/**
 * A rule's apply (docs/features/auto-feed.md "The UI", Applied by rule): the line, the commit's 12 hex
 * and the rule hash's 12, each whole as its title, the hash whole as what a screen reader says. Only
 * a document holding `delegated` shows it.
 */
function AppliedByRule({ rule, commit }: { rule: string; commit: string }) {
  const t = useTranslate();
  return (
    <p className="note by-rule" data-delegated="true">
      <Icon name="terminal" />
      <span>
        {fill(t("inbox.byRule", { hash: slot("hash"), commit: slot("commit") }), {
          hash: (
            <code title={rule}>
              <span aria-hidden="true">{shownSlot(rule.slice(0, 15))}</span>
              <span className="sr-only">{shownSlot(rule)}</span>
            </code>
          ),
          commit: <code title={commit}>{shownSlot(commit.slice(0, 12))}</code>,
        })}
      </span>
    </p>
  );
}

/**
 * A routed proposal the inbox no longer lists, read on its own (docs/features/inbox-closed-link.md
 * "Description and interactions"): an `IssueView` pane as the Inbox's card is, saying it is shown to
 * read, with no decision bar. The trail; the gist, else the summary, the target, the kind; the status;
 * then who decided it and when, its commit, its record and its targets, a row left out when the
 * document holds none; and the decision's note. Served text is text, never markup.
 */
export function ClosedProposal({ project, proposal }: { project: string; proposal: Proposal }) {
  const t = useTranslate();
  const titleId = useId();
  const projectName = useProjectName(project);
  const id = proposal.id ?? "";
  const crumbs: Crumb[] = [
    { key: "project", label: projectName, href: homeHash(project) },
    { key: "inbox", label: t("app.sections.inbox.label"), href: sectionHash(project, "inbox") },
    { key: "proposal", label: id, mono: true },
  ];
  const title = written(proposal.gist) ?? written(proposal.summary) ?? proposal.target_id ?? proposal.kind ?? id;
  const targets = targetsOf(proposal);
  const record = proposal.record_id ?? proposal.record_path;
  const rows: (Property | null)[] = [
    proposal.kind === null
      ? null
      : {
          key: "kind",
          label: t("inbox.card.kind"),
          value: (
            <span className="mono" lang="en">
              {shownSlot(proposal.kind)}
            </span>
          ),
        },
    proposal.decided_by === null ? null : { key: "decidedBy", label: t("inbox.closed.decidedBy"), value: <Decider text={proposal.decided_by} /> },
    proposal.decided_at === null
      ? null
      : { key: "decidedAt", label: t("inbox.closed.decidedAt"), value: <time dateTime={proposal.decided_at}>{formatUtc(proposal.decided_at, t)}</time> },
    proposal.applied_commit === null ? null : { key: "commit", label: t("inbox.closed.commit"), value: <span className="mono">{shownSlot(proposal.applied_commit)}</span> },
    record === null
      ? null
      : {
          key: "record",
          label: t("inbox.closed.record"),
          value: (
            <DocLink project={project} docRef={record}>
              <bdi>{shownSlot(proposal.record_title ?? record)}</bdi>
            </DocLink>
          ),
        },
    targets.length === 0
      ? null
      : {
          key: "targets",
          label: t("inbox.closed.targets"),
          value: (
            <ul className="closed-targets">
              {targets.map((target) => (
                <li key={target}>
                  <DocLink project={project} docRef={target}>
                    <bdi className="mono">{shownSlot(target)}</bdi>
                  </DocLink>
                </li>
              ))}
            </ul>
          ),
        },
  ];
  return (
    <article className="card" aria-labelledby={titleId} data-closed={id}>
      <IssueView
        layout="pane"
        breadcrumbs={<Breadcrumbs items={crumbs} />}
        title={
          <h2 id={titleId} className="card-title">
            <bdi>{shownSlot(title)}</bdi>
          </h2>
        }
        meta={proposal.status === null ? null : <Lozenge {...statusLozenge(proposal.status, t)} />}
        actions={
          <>
            <p className="note" role="status">
              <Icon name="info" />
              <span>{t("inbox.closed.notice")}</span>
            </p>
            {proposal.delegated !== undefined && <AppliedByRule rule={proposal.delegated.rule} commit={proposal.delegated.commit} />}
          </>
        }
        side={<PropertyList label={t("inbox.card.details")} items={rows.filter((row) => row !== null)} />}
        main={
          proposal.decision_note === null ? null : (
            <Section title={t("inbox.closed.note")}>
              <p className="board-text">{shownText(proposal.decision_note)}</p>
            </Section>
          )
        }
      />
    </article>
  );
}
