import { lazy, Suspense, useEffect, useRef, useState } from "react";
import { apiErrorOf } from "../api/client";
import { useNode, useProjectName, type ProposalQuery } from "../api/queries";
import type { Choice, InboxEntry, Proposal, ProposalOption } from "../api/types";
import { homeHash, sectionHash } from "../app/routes";
import { DocLink } from "../doc/DocLink";
import { ActOff } from "../board/ActOff";
import { Prose } from "../markdown/Prose";
import { DEFAULT_TEXT_MODE, TextModeSwitch, type TextMode } from "../markdown/TextMode";
import { Badge } from "../ui/Badge";
import { Breadcrumbs, type Crumb } from "../ui/Breadcrumbs";
import { Button } from "../ui/Button";
import { DiffView } from "../ui/DiffView";
import { focusIsLost } from "../ui/focus";
import { Icon } from "../ui/Icon";
import { IssueView } from "../ui/IssueView";
import { Lozenge } from "../ui/Lozenge";
import { PropertyList, type Property } from "../ui/PropertyList";
import { shownConfirmText, shownSlot } from "../ui/text";
import { ErrorPanel, Section, Skeleton } from "../ui/states";
import { formatAge, formatUtc } from "../ui/time";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { DECISION_KEYS, isStaged, verbOf, type DecisionKind } from "./decisions";
import { DriftSection, TerminalDecision } from "./DriftItem";
import { AskedFold, AskedText, LeadMatters, RecommendedMark } from "./ItemLead";
import { gapLabel, previewLabel, severityLook, statusLook, statusLozenge } from "./labels";
import { isRefused } from "./revisions";
import { RevisionList } from "./RevisionList";
import { leadingGist, summaryOf } from "./summary";
import { targetsOf } from "./targets";
import { useTranslate, type Translate } from "../i18n";
import { fill, slot } from "../i18n/fill";

/**
 * The staged choice arrives in a chunk of its own, loaded only for a proposal with one staged
 * (`docs/canon/decision-staging.md` "UI"); until it does, the card says when it was staged.
 */
const StagedChoice = lazy(() => import("./StagedChoice").then((module) => ({ default: module.StagedChoice })));

/**
 * An interpretation's section arrives in a chunk of its own too, loaded only for a proposal whose
 * review document carries one (`docs/features/kind-interpretation.md` "Data"); until it does, its
 * heading over a skeleton.
 */
const InterpretationSection = lazy(() => import("./InterpretationSection").then((module) => ({ default: module.InterpretationSection })));

/** The target as a document: the modal over the Inbox (docs/features/doc-modal.md "Acceptance criteria", DM-8). */
function OpenInTree({ project, id }: { project: string; id: string }) {
  const t = useTranslate();
  return (
    <p className="target-open">
      <DocLink project={project} docRef={id}>
        {fill(t("inbox.card.openDoc", { id: slot("id") }), { id: <span className="sr-only">{t("inbox.card.openDocId", { id })}</span> })}
      </DocLink>
    </p>
  );
}

/**
 * One target and its current section, rendered or as its source, with its own error and Retry.
 * After a successful retry the Retry that had focus is gone; focus goes to the target's head
 * instead of the page's body, so the queue's keys keep working.
 */
function TargetNode({ project, id, mode, onMode }: { project: string; id: string; mode: TextMode; onMode: (mode: TextMode) => void }) {
  const t = useTranslate();
  const query = useNode(project, id);
  const failure = useRetainedFailure(query.error, query.isFetching);
  const head = useRef<HTMLParagraphElement>(null);
  const retried = useRef(false);

  useEffect(() => {
    if (!retried.current || query.data === undefined) {
      return;
    }
    retried.current = false;
    if (focusIsLost()) {
      head.current?.focus();
    }
  }, [query.data]);

  if (query.data === undefined) {
    return (
      <li className="target">
        <p className="target-head mono">{id}</p>
        {failure === null ? (
          <Skeleton label={t("inbox.card.sectionLoading", { id })} lines={2} />
        ) : (
          <ErrorPanel
            title={t("inbox.card.sectionFailed", { id })}
            message={apiErrorOf(failure).message}
            retrying={query.isFetching}
            attempt={query.errorUpdateCount}
            onRetry={() => {
              retried.current = true;
              void query.refetch({ cancelRefetch: false });
            }}
          />
        )}
      </li>
    );
  }
  const view = query.data;
  if (view.nodes.length === 0) {
    return (
      <li className="target">
        <p ref={head} className="target-head mono" tabIndex={-1}>
          {id}
        </p>
        {view.reason === null ? (
          <p className="muted">{t("inbox.card.noNode")}</p>
        ) : (
          <p className="muted" lang="en">
            {view.reason}
          </p>
        )}
        <OpenInTree project={project} id={id} />
      </li>
    );
  }
  return (
    <>
      {view.nodes.map((node, index) => (
        <li key={`${node.path}:${String(node.line)}`} className="target">
          <p ref={index === 0 ? head : undefined} className="target-head" tabIndex={-1}>
            <span className="mono">{node.id ?? id}</span>
            <span className="kind-tag">{node.kind ?? t("inbox.card.noKind")}</span>
            {node.title !== null && <span className="target-title">{node.title}</span>}
          </p>
          <p className="target-path mono">
            {node.path}:{node.line}
            {node.rev !== null && t("inbox.card.rev", { rev: node.rev })}
          </p>
          <div className="target-label-row">
            <p className="target-label">{t("inbox.card.currentSection")}</p>
            <TextModeSwitch mode={mode} label={t("inbox.card.showSectionAs", { id: node.id ?? id })} onChange={onMode} />
          </div>
          {mode === "source" ? (
            <pre className="node-text">{node.text}</pre>
          ) : (
            <Prose
              className="node-text target-markdown"
              source={node.text}
              links={null}
              project={project}
              baseLevel={3}
              frontMatter={node.line === 1}
              firstLine={node.line}
            />
          )}
          {node.truncated && node.omitted !== null && (
            <p className="muted">{t("inbox.card.cut", { from: node.omitted.lines[0], to: node.omitted.lines[1] })}</p>
          )}
          {index === 0 && <OpenInTree project={project} id={id} />}
        </li>
      ))}
    </>
  );
}

/** The owner's choice a decision record holds, in words: an option by its label, the working answer, an answer. */
function choiceText(choice: Choice, options: readonly ProposalOption[], t: Translate): string {
  if ("option" in choice) {
    const label = options[choice.option]?.label;
    return label === undefined ? t("inbox.card.option", { index: choice.option }) : t("inbox.card.optionLabel", { index: choice.option, label });
  }
  if ("answer" in choice) {
    return choice.answer;
  }
  return t("inbox.staged.workingAnswer");
}

/** A review's options, each with its effect and price, the recommended one marked with who made it; never picked here. */
function OptionsSection({ project, review }: { project: string; review: Proposal }) {
  const t = useTranslate();
  return (
    <Section title={t("inbox.card.options")}>
      {review.options.length === 0 ? (
        <p className="muted">{t("inbox.card.noOptions")}</p>
      ) : (
        <ol className="option-list">
          {review.options.map((choice, index) => {
            const recommended = index === review.recommendation;
            return (
              <li key={`${String(index)}-${choice.label}`} className={recommended ? "option option-recommended" : "option"}>
                <p className="option-label">
                  {/* Agent text, as the round sheet writes a slot: controls as `\u{..}`, isolated. */}
                  <span>
                    <bdi>{shownSlot(choice.label)}</bdi>
                  </span>
                  {recommended && <RecommendedMark author={review.author} />}
                </p>
                <Prose className="option-effect" source={choice.effect} links={null} project={project} baseLevel={3} frontMatter={false} />
                <div className="option-price">
                  <p className="price-label">{t("inbox.card.price")}</p>
                  <Prose source={choice.price} links={null} project={project} baseLevel={3} frontMatter={false} />
                </div>
              </li>
            );
          })}
        </ol>
      )}
    </Section>
  );
}

/** A review's working answer with the price of another answer, then a decision's why. */
function AnswerSections({ project, review }: { project: string; review: Proposal }) {
  const t = useTranslate();
  return (
    <>
      {review.working_answer !== null && (
        <Section title={t("inbox.card.workingAnswer")}>
          <Prose className="prose" source={review.working_answer} links={null} project={project} baseLevel={3} frontMatter={false} />
          {review.price_of_other !== null && (
            <div className="prose">
              <p className="price-label">{t("inbox.card.priceOfOther")}</p>
              <Prose source={review.price_of_other} links={null} project={project} baseLevel={3} frontMatter={false} />
            </div>
          )}
        </Section>
      )}

      {/* A decision's why, as its author wrote it (docs/features/card-questions.md "Acceptance criteria", U5). */}
      {review.why !== null && (
        <Section title={t("inbox.proposal.why")}>
          <Prose className="prose" source={review.why} links={null} project={project} baseLevel={3} frontMatter={false} />
        </Section>
      )}
    </>
  );
}

/**
 * A question's lead on the Inbox card (docs/features/plain-questions.md "Acceptance criteria", PQ12),
 * its gist the card's title: why it matters, its options, then what it said before it had a gist, its
 * text verbatim with its working answer, price and why, folded and collapsed; the targets follow.
 */
function QuestionLead({ project, review }: { project: string; review: Proposal }) {
  return (
    <>
      {review.matters !== null && <LeadMatters matters={review.matters} />}
      <OptionsSection project={project} review={review} />
      <AskedFold>
        {review.summary !== null && <AskedText text={review.summary} />}
        <AnswerSections project={project} review={review} />
      </AskedFold>
    </>
  );
}

/** The review document's sections, as `spec review` sent them. */
function ReviewBody({
  project,
  review,
  others,
  now,
  onSelect,
}: {
  project: string;
  review: Proposal;
  others: readonly InboxEntry[];
  now: number;
  onSelect: (id: string) => void;
}) {
  const t = useTranslate();
  const preview = previewLabel(review.preview, t);
  const author = review.author;
  // A hub's alone, the request's login of who raised it; absent: no row (`docs/features/hub-mcp.md` "Identity").
  const login = author?.login;
  const diagnostics = review.diagnostics ?? [];
  // A drift item (its `drift` key, never its kind): its evidence first; no options, no section diff.
  const drift = review.drift;
  return (
    <>
      {drift !== null && <DriftSection project={project} drift={drift} commit={review.base_commit} />}

      {review.evidence.length > 0 && (
        <Section title={t("inbox.card.evidence")}>
          <ul className="evidence-list">
            {review.evidence.map((item, index) => (
              <li key={`${String(index)}-${item.file}`} className="evidence">
                <p className="mono">
                  {item.file}
                  {item.lines !== null && `:${item.lines}`}
                </p>
                {item.qpath !== null && <p className="mono muted">{item.qpath}</p>}
                <dl className="pairs">
                  <dt>{t("inbox.card.observed")}</dt>
                  <dd>{item.observed}</dd>
                  <dt>{t("inbox.card.documented")}</dt>
                  <dd>{item.documented}</dd>
                </dl>
              </li>
            ))}
          </ul>
        </Section>
      )}

      {/* A drift item has no options to choose: decided on a terminal, its section left out. A gist leads with them instead (QuestionLead). */}
      {drift === null && review.gist === null && <OptionsSection project={project} review={review} />}
      {review.gist === null && <AnswerSections project={project} review={review} />}

      {review.rationale !== null && review.rationale !== review.summary && (
        <Section title={t("inbox.card.rationale")}>
          <Prose className="prose" source={review.rationale} links={null} project={project} baseLevel={3} frontMatter={false} />
        </Section>
      )}

      {/* The key's presence alone decides it, never the kind (ADR-0031). */}
      {review.interpretation !== null && (
        <Suspense
          fallback={
            <Section title={t("inbox.interpretation.title")}>
              <Skeleton label={t("inbox.interpretation.loading")} lines={3} />
            </Section>
          }
        >
          <InterpretationSection project={project} interpretation={review.interpretation} />
        </Suspense>
      )}

      {review.record_id !== null && (
        <Section title={t("inbox.card.record")}>
          <dl className="pairs">
            <dt>{t("inbox.card.recordId")}</dt>
            <dd className="mono">{review.record_id}</dd>
            {review.record_title !== null && (
              <>
                <dt>{t("inbox.card.title")}</dt>
                <dd>{review.record_title}</dd>
              </>
            )}
            {review.record_path !== null && (
              <>
                <dt>{t("inbox.card.file")}</dt>
                <dd className="mono">{review.record_path}</dd>
              </>
            )}
            {review.choice !== null && (
              <>
                <dt>{t("inbox.card.yourChoice")}</dt>
                <dd>{choiceText(review.choice, review.options, t)}</dd>
              </>
            )}
          </dl>
        </Section>
      )}

      <Section title={t("inbox.card.provenance")}>
        <dl className="pairs">
          <dt>{t("inbox.card.author")}</dt>
          <dd>
            {author === null
              ? t("inbox.card.unknown")
              : [author.type, author.role === null ? null : t("inbox.card.role", { role: author.role })].filter((part) => part !== null).join(", ")}
          </dd>
          {login !== undefined && (
            <>
              <dt>{t("inbox.card.login")}</dt>
              {/* Data, as a Confirm dialog shows it: every control and format character `\u{..}`, isolated. */}
              <dd className="mono">
                <bdi>{shownConfirmText(login)}</bdi>
              </dd>
            </>
          )}
          <dt>{t("inbox.card.model")}</dt>
          <dd className="mono">{author?.model ?? t("inbox.card.unknown")}</dd>
          {author !== null && author.run !== null && (
            <>
              <dt>{t("inbox.card.run")}</dt>
              <dd className="mono">{author.run}</dd>
            </>
          )}
          {review.created_at !== null && (
            <>
              <dt>{t("inbox.card.raised")}</dt>
              <dd>
                <time dateTime={review.created_at}>
                  {formatAge(review.created_at, now, t)} ({formatUtc(review.created_at, t)})
                </time>
              </dd>
            </>
          )}
          {review.branch !== null && (
            <>
              <dt>{t("inbox.card.branch")}</dt>
              <dd className="mono">{review.branch}</dd>
            </>
          )}
          {review.worktree !== null && (
            <>
              <dt>{t("inbox.card.worktree")}</dt>
              <dd className="mono">{review.worktree}</dd>
            </>
          )}
          {review.linked !== null && (
            <>
              <dt>{t("inbox.card.linked")}</dt>
              <dd className="mono">
                <a href={sectionHash(project, "inbox", review.linked)}>{review.linked}</a>
              </dd>
            </>
          )}
          {review.distinct_from.length > 0 && (
            <>
              <dt>{t("inbox.card.distinctFrom")}</dt>
              <dd className="mono">{review.distinct_from.join(", ")}</dd>
            </>
          )}
          {review.decision_note !== null && (
            <>
              <dt>{t("inbox.card.decisionNote")}</dt>
              <dd>
                <Prose source={review.decision_note} links={null} project={project} baseLevel={3} frontMatter={false} />
              </dd>
            </>
          )}
        </dl>
      </Section>

      {others.length > 0 && (
        <Section title={t("inbox.card.others")}>
          <p className="muted">{t("inbox.card.othersNote")}</p>
          <ul className="other-list">
            {others.map((other) => (
              <li key={other.id}>
                <button
                  type="button"
                  className="link-button mono"
                  onClick={() => {
                    onSelect(other.id);
                  }}
                >
                  {other.id}
                </button>
                <Badge name={t("inbox.statusName")} look={statusLook(other.status, t)} />
                <span>{summaryOf(other, t)}</span>
              </li>
            ))}
          </ul>
        </Section>
      )}

      {/* A drift item changes no spec text: no diff, preview or conflict to show (the key decides, never the kind). */}
      {drift === null && (
        <Section title={t("inbox.card.diff")}>
          <DiffView diff={review.diff} />
          {preview !== null && <p className="muted">{t("inbox.card.preview", { preview })}</p>}
          {review.conflict !== null && (
            <>
              <p className="target-label">{t("inbox.card.conflict")}</p>
              <pre className="node-text">{review.conflict}</pre>
            </>
          )}
        </Section>
      )}

      {/* As the review document lists them after `conflict`; null: no section (the key decides, never the kind). */}
      {review.revisions !== null && (
        <Section title={t("inbox.card.revisions")}>
          {review.revisions.length === 0 ? (
            <p className="muted">{t("inbox.card.noRevisions")}</p>
          ) : (
            <>
              <p className="muted">{review.revisions.some(isRefused) ? t("inbox.card.revisionsRefused") : t("inbox.card.revisionsRaised")}</p>
              <RevisionList entries={review.revisions} />
            </>
          )}
        </Section>
      )}

      {diagnostics.length > 0 && (
        <Section title={t("inbox.card.findings")}>
          <ul className="finding-list">
            {diagnostics.map((finding, index) => (
              <li key={`${String(index)}-${finding.code}`}>
                <span className="mono">
                  {finding.severity} {finding.code}
                </span>{" "}
                <span className="mono muted">
                  {finding.path}:{finding.line}
                </span>{" "}
                <span lang="en">{finding.message}</span>
              </li>
            ))}
          </ul>
        </Section>
      )}

      {review.notes.length > 0 && (
        <Section title={t("inbox.card.notes")}>
          <ul className="note-list">
            {review.notes.map((note) => (
              <li key={note} lang="en">
                {note}
              </li>
            ))}
          </ul>
        </Section>
      )}
    </>
  );
}

/**
 * One proposal: its inbox entry at once, then its review document (`getProposal`): loading, the
 * daemon's error with Retry, the exit-1 document's notes, or every section it sends. A decision
 * needs the document (its options and `updated_at`); until it is read the buttons do nothing. A
 * staged choice shows under the decisions, with its terminal command and Unstage. A drift item
 * (the document's `drift`) offers no decision to stage: its terminal commands stand in their place,
 * and it has no options or section diff to show. Where a hub's `allowed` omits `stage`, the decisions
 * that stage and Unstage stay focusable and do nothing; the two that send nothing stay as they are.
 * Laid out as a work item beside the queue (docs/features/design-sweep.md "Acceptance criteria", DS5):
 * the trail, the summary, its status as a lozenge, the decisions; then its facts, then the review. A
 * question holding a gist is titled by it and leads with why it matters and its options, its text
 * folded under them (docs/features/plain-questions.md "Acceptance criteria", PQ12).
 */
export function ProposalCard({
  project,
  entry,
  review,
  others,
  now,
  deciding,
  canStage,
  unstageRefusal,
  onDecide,
  onUnstage,
  onConfirm,
  onSelect,
}: {
  project: string;
  entry: InboxEntry;
  /** The review document's read (the Inbox's useProposal). */
  review: ProposalQuery;
  /** Other proposals in the queue on the same nodes: information only, nothing waits on them. */
  others: readonly InboxEntry[];
  now: number;
  /** A stage or an unstage is being sent: the buttons stay focusable but do nothing. */
  deciding: boolean;
  /** False: a hub's `allowed` omits `stage`. */
  canStage: boolean;
  /** The daemon's words when it refused to unstage this proposal. */
  unstageRefusal: string | null;
  onDecide: (kind: DecisionKind) => void;
  onUnstage: () => void;
  /** Opens the Confirm dialog on this proposal; absent: no control (`docs/features/hub-confirm.md`). */
  onConfirm: (() => void) | undefined;
  onSelect: (id: string) => void;
}) {
  const t = useTranslate();
  const projectName = useProjectName(project);
  const failure = useRetainedFailure(review.error, review.isFetching);
  const body = useRef<HTMLDivElement>(null);
  const retried = useRetryFocus(review.data, () => body.current);
  const targets = targetsOf(entry);
  const reviewed = review.data?.id === null ? undefined : review.data;
  // A stage is sent against the document's `updated_at`: without one there is nothing to stage on.
  const ready = reviewed !== undefined && reviewed.updated_at !== null;
  const id = entry.id;
  // Rendered or Source for the targets' current sections, kept from proposal to proposal.
  const [textMode, setTextMode] = useState<TextMode>(DEFAULT_TEXT_MODE);

  let content;
  if (review.data === undefined) {
    content =
      failure === null ? (
        <div aria-busy="true">
          <Skeleton label={t("inbox.card.reviewLoading", { id })} lines={4} />
        </div>
      ) : (
        <ErrorPanel
          title={t("inbox.card.reviewFailed", { id })}
          message={apiErrorOf(failure).message}
          retrying={review.isFetching}
          attempt={review.errorUpdateCount}
          onRetry={() => {
            retried();
            void review.refetch({ cancelRefetch: false });
          }}
        />
      );
  } else if (reviewed === undefined) {
    content = (
      <div className="notice">
        <p className="notice-title">
          <Icon name="info" />
          <span>{t("inbox.card.noReview", { id })}</span>
        </p>
        <ul className="note-list">
          {review.data.notes.map((note) => (
            <li key={note} lang="en">
              {note}
            </li>
          ))}
        </ul>
      </div>
    );
  } else {
    content = <ReviewBody project={project} review={reviewed} others={others} now={now} onSelect={onSelect} />;
  }

  const staged = reviewed !== undefined && reviewed.id === id && reviewed.staged !== null && reviewed.staged_at !== null ? reviewed : null;
  /** The gist the card is titled by, the agent's words as the lead draws them; null: the summary's line as before. */
  const titleGist = leadingGist(reviewed ?? entry);
  // One primary per item: a staged choice confirmed here takes it from Accept, as a task's Answer dialog does.
  const acceptLook = staged !== null && onConfirm !== undefined ? "default" : "primary";
  const crumbs: Crumb[] = [
    { key: "project", label: projectName, href: homeHash(project) },
    { key: "inbox", label: t("app.sections.inbox.label"), href: sectionHash(project, "inbox") },
    { key: "proposal", label: id, mono: true },
  ];
  const details: Property[] = [
    { key: "kind", label: t("inbox.card.kind"), value: <span className="mono">{reviewed?.kind ?? entry.kind}</span> },
    ...(reviewed === undefined ? [] : [{ key: "gap", label: t("inbox.card.gapType"), value: gapLabel(reviewed.gap_type, t) ?? t("inbox.card.none") }]),
    { key: "severity", label: t("inbox.severityName"), value: <Badge look={severityLook(reviewed === undefined ? entry.severity : reviewed.severity, t)} /> },
    // Only the review document names the task (an inbox entry has no `task_id`): absent until it is read.
    ...(reviewed === undefined
      ? []
      : [
          {
            key: "task",
            label: t("inbox.card.task"),
            value: reviewed.task_id === null ? t("inbox.card.noTask") : <a className="mono" href={sectionHash(project, "tasks", reviewed.task_id)}>{reviewed.task_id}</a>,
          },
        ]),
    // The task it was raised for, its card on the board (docs/canon/card-questions.md "The rule"); only the review document names it.
    ...(reviewed === undefined || reviewed.card_id === null
      ? []
      : [
          {
            key: "board",
            label: t("app.sections.board.label"),
            value: (
              <a href={sectionHash(project, "board", reviewed.card_id)}>
                {fill(t("inbox.proposal.task", { id: slot("id") }), { id: <span className="mono">{shownSlot(reviewed.card_id)}</span> })}
              </a>
            ),
          },
        ]),
  ];

  return (
    <article className="card" aria-labelledby="card-title">
      <IssueView
        layout="pane"
        breadcrumbs={<Breadcrumbs items={crumbs} />}
        title={
          <h2 id="card-title" className="card-title">
            {titleGist === null ? summaryOf(reviewed ?? entry, t) : <bdi>{shownSlot(titleGist)}</bdi>}
          </h2>
        }
        meta={<Lozenge {...statusLozenge(reviewed?.status ?? entry.status, t)} />}
        actions={
          <>
            {reviewed !== undefined && reviewed.drift !== null ? (
              <TerminalDecision id={id} worktree={reviewed.worktree} />
            ) : (
              <>
                <div className="decision-bar" role="group" aria-label={t("inbox.card.decide", { id })}>
                  {DECISION_KEYS.map(([kind, key]) => {
                    const idle = deciding || !ready || (isStaged(kind) && !canStage);
                    return (
                      <Button
                        key={kind}
                        appearance={kind === "accept" ? acceptLook : kind === "reject" ? "danger" : "default"}
                        aria-keyshortcuts={key}
                        aria-disabled={idle}
                        onClick={() => {
                          if (!idle) {
                            onDecide(kind);
                          }
                        }}
                      >
                        <span>{verbOf(kind, t)}</span>
                        <kbd aria-hidden="true">{key}</kbd>
                      </Button>
                    );
                  })}
                </div>
                {!canStage && <ActOff act="stage" />}
              </>
            )}
            {staged !== null && staged.staged_at !== null && (
              <Suspense
                fallback={
                  <p className="staged-pending" aria-busy="true">
                    {fill(t(onConfirm === undefined ? "inbox.card.stagedTerminal" : "inbox.card.stagedHere", { at: slot("at") }), {
                      at: <time dateTime={staged.staged_at}>{formatUtc(staged.staged_at, t)}</time>,
                    })}
                  </p>
                }
              >
                <StagedChoice id={id} review={staged} busy={deciding || !canStage} refusal={unstageRefusal} onUnstage={onUnstage} onConfirm={onConfirm} confirmBusy={deciding} />
              </Suspense>
            )}
          </>
        }
        side={<PropertyList label={t("inbox.card.details")} items={details} />}
        main={
          <div className="card-body" ref={body} tabIndex={-1}>
            {reviewed !== undefined && reviewed.gist !== null && <QuestionLead project={project} review={reviewed} />}
            <Section title={t("inbox.card.targets")}>
              {targets.length === 0 ? (
                <p className="muted">{t("inbox.card.noTargets")}</p>
              ) : (
                <ul className="target-list">
                  {targets.map((target) => (
                    <TargetNode key={target} project={project} id={target} mode={textMode} onMode={setTextMode} />
                  ))}
                </ul>
              )}
            </Section>
            {content}
          </div>
        }
      />
    </article>
  );
}
