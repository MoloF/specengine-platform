import { useEffect, useRef } from "react";
import { apiErrorOf } from "../api/client";
import { useNode, type ProposalQuery } from "../api/queries";
import type { Choice, InboxEntry, Proposal, ProposalOption } from "../api/types";
import { sectionHash } from "../app/routes";
import { Badge } from "../ui/Badge";
import { DiffView } from "../ui/DiffView";
import { focusIsLost } from "../ui/focus";
import { Icon } from "../ui/Icon";
import { ErrorPanel, Section, Skeleton } from "../ui/states";
import { formatAge, formatUtc } from "../ui/time";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { DECISION_KEYS, WORDING, type DecisionKind } from "./decisions";
import { gapLabel, previewLabel, severityLook, statusLook } from "./labels";
import { summaryOf } from "./summary";
import { targetsOf } from "./targets";

/** The target in the Spec tree: its text, links, bundle and other proposals. */
function OpenInTree({ project, id }: { project: string; id: string }) {
  return (
    <p className="target-open">
      <a href={sectionHash(project, "tree", id)}>
        Open in spec tree<span className="sr-only">: {id}</span>
      </a>
    </p>
  );
}

/**
 * One target and its current section, with its own error and Retry. After a successful retry the
 * Retry that had focus is gone; focus goes to the target's head instead of the page's body, so the
 * queue's keys keep working.
 */
function TargetNode({ project, id }: { project: string; id: string }) {
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
          <Skeleton label={`Loading the current section of ${id}`} lines={2} />
        ) : (
          <ErrorPanel
            title={`The current section of ${id} could not be read`}
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
        <p className="muted">{view.reason ?? "No node answers this ID."}</p>
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
            <span className="kind-tag">{node.kind ?? "no kind"}</span>
            {node.title !== null && <span className="target-title">{node.title}</span>}
          </p>
          <p className="target-path mono">
            {node.path}:{node.line}
            {node.rev !== null && ` | rev ${String(node.rev)}`}
          </p>
          <p className="target-label">Current section</p>
          <pre className="node-text">{node.text}</pre>
          {node.truncated && node.omitted !== null && (
            <p className="muted">
              Cut at the output cap: lines {node.omitted.lines[0]}-{node.omitted.lines[1]} not shown.
            </p>
          )}
          {index === 0 && <OpenInTree project={project} id={id} />}
        </li>
      ))}
    </>
  );
}

/** The owner's choice a decision record holds, in words: an option by its label, the working answer, an answer. */
function choiceText(choice: Choice, options: readonly ProposalOption[]): string {
  if ("option" in choice) {
    const label = options[choice.option]?.label;
    return label === undefined ? `Option ${String(choice.option)}` : `Option ${String(choice.option)}: ${label}`;
  }
  if ("answer" in choice) {
    return choice.answer;
  }
  return "The working answer";
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
  const preview = previewLabel(review.preview);
  const author = review.author;
  const diagnostics = review.diagnostics ?? [];
  return (
    <>
      {review.evidence.length > 0 && (
        <Section title="Evidence">
          <ul className="evidence-list">
            {review.evidence.map((item, index) => (
              <li key={`${String(index)}-${item.file}`} className="evidence">
                <p className="mono">
                  {item.file}
                  {item.lines !== null && `:${item.lines}`}
                </p>
                {item.qpath !== null && <p className="mono muted">{item.qpath}</p>}
                <dl className="pairs">
                  <dt>Observed in the code</dt>
                  <dd>{item.observed}</dd>
                  <dt>Documented in the spec</dt>
                  <dd>{item.documented}</dd>
                </dl>
              </li>
            ))}
          </ul>
        </Section>
      )}

      <Section title="Options">
        {review.options.length === 0 ? (
          <p className="muted">No options attached.</p>
        ) : (
          <ol className="option-list">
            {review.options.map((choice, index) => {
              const recommended = index === review.recommendation;
              return (
                <li key={`${String(index)}-${choice.label}`} className={recommended ? "option option-recommended" : "option"}>
                  <p className="option-label">
                    <span>{choice.label}</span>
                    {recommended && (
                      <span className="recommended-mark">
                        <Icon name="recommended" />
                        Recommended
                      </span>
                    )}
                  </p>
                  <p>{choice.effect}</p>
                  <p>
                    <span className="price-label">Price:</span> {choice.price}
                  </p>
                </li>
              );
            })}
          </ol>
        )}
      </Section>

      {review.working_answer !== null && (
        <Section title="Working answer">
          <p className="prose">{review.working_answer}</p>
          {review.price_of_other !== null && (
            <p className="prose">
              <span className="price-label">Price of another answer:</span> {review.price_of_other}
            </p>
          )}
        </Section>
      )}

      {review.rationale !== null && review.rationale !== review.summary && (
        <Section title="Rationale">
          <p className="prose">{review.rationale}</p>
        </Section>
      )}

      {review.record_id !== null && (
        <Section title="Decision record">
          <dl className="pairs">
            <dt>Record</dt>
            <dd className="mono">{review.record_id}</dd>
            {review.record_title !== null && (
              <>
                <dt>Title</dt>
                <dd>{review.record_title}</dd>
              </>
            )}
            {review.record_path !== null && (
              <>
                <dt>File</dt>
                <dd className="mono">{review.record_path}</dd>
              </>
            )}
            {review.choice !== null && (
              <>
                <dt>Your choice</dt>
                <dd>{choiceText(review.choice, review.options)}</dd>
              </>
            )}
          </dl>
        </Section>
      )}

      <Section title="Provenance">
        <dl className="pairs">
          <dt>Author</dt>
          <dd>
            {author === null
              ? "Unknown"
              : [author.type, author.role === null ? null : `role ${author.role}`].filter((part) => part !== null).join(", ")}
          </dd>
          <dt>Model</dt>
          <dd className="mono">{author?.model ?? "Unknown"}</dd>
          {author !== null && author.run !== null && (
            <>
              <dt>Run</dt>
              <dd className="mono">{author.run}</dd>
            </>
          )}
          {review.created_at !== null && (
            <>
              <dt>Raised</dt>
              <dd>
                <time dateTime={review.created_at}>
                  {formatAge(review.created_at, now)} ({formatUtc(review.created_at)})
                </time>
              </dd>
            </>
          )}
          {review.branch !== null && (
            <>
              <dt>Branch</dt>
              <dd className="mono">{review.branch}</dd>
            </>
          )}
          {review.worktree !== null && (
            <>
              <dt>Worktree</dt>
              <dd className="mono">{review.worktree}</dd>
            </>
          )}
          {review.linked !== null && (
            <>
              <dt>Linked proposal</dt>
              <dd className="mono">
                <a href={sectionHash(project, "inbox", review.linked)}>{review.linked}</a>
              </dd>
            </>
          )}
          {review.distinct_from.length > 0 && (
            <>
              <dt>Raised as distinct from</dt>
              <dd className="mono">{review.distinct_from.join(", ")}</dd>
            </>
          )}
          {review.decision_note !== null && (
            <>
              <dt>Last decision note</dt>
              <dd>{review.decision_note}</dd>
            </>
          )}
        </dl>
      </Section>

      {others.length > 0 && (
        <Section title="Also open on these nodes">
          <p className="muted">Information only: each proposal is decided on its own and rebased when applied.</p>
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
                <Badge name="Status" look={statusLook(other.status)} />
                <span>{summaryOf(other)}</span>
              </li>
            ))}
          </ul>
        </Section>
      )}

      <Section title="Section diff">
        <DiffView diff={review.diff} />
        {preview !== null && <p className="muted">Apply preview: {preview}.</p>}
        {review.conflict !== null && (
          <>
            <p className="target-label">Conflict at apply</p>
            <pre className="node-text">{review.conflict}</pre>
          </>
        )}
      </Section>

      {diagnostics.length > 0 && (
        <Section title="Check findings the change introduces">
          <ul className="finding-list">
            {diagnostics.map((finding, index) => (
              <li key={`${String(index)}-${finding.code}`}>
                <span className="mono">
                  {finding.severity} {finding.code}
                </span>{" "}
                <span className="mono muted">
                  {finding.path}:{finding.line}
                </span>{" "}
                <span>{finding.message}</span>
              </li>
            ))}
          </ul>
        </Section>
      )}

      {review.notes.length > 0 && (
        <Section title="Notes">
          <ul className="note-list">
            {review.notes.map((note) => (
              <li key={note}>{note}</li>
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
 * needs the document (its options); until it is read the buttons do nothing.
 */
export function ProposalCard({
  project,
  entry,
  review,
  others,
  now,
  deciding,
  onDecide,
  onSelect,
}: {
  project: string;
  entry: InboxEntry;
  /** The review document's read (the Inbox's useProposal). */
  review: ProposalQuery;
  /** Other proposals in the queue on the same nodes: information only, nothing waits on them. */
  others: readonly InboxEntry[];
  now: number;
  /** A decision is being sent: the buttons stay focusable but do nothing. */
  deciding: boolean;
  onDecide: (kind: DecisionKind) => void;
  onSelect: (id: string) => void;
}) {
  const failure = useRetainedFailure(review.error, review.isFetching);
  const body = useRef<HTMLDivElement>(null);
  const retried = useRetryFocus(review.data, () => body.current);
  const targets = targetsOf(entry);
  const reviewed = review.data?.id === null ? undefined : review.data;
  const ready = reviewed !== undefined;
  const id = entry.id;

  let content;
  if (review.data === undefined) {
    content =
      failure === null ? (
        <div aria-busy="true">
          <Skeleton label={`Loading the review of ${id}`} lines={4} />
        </div>
      ) : (
        <ErrorPanel
          title={`The review of ${id} could not be read`}
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
          <span>{id} has no review document in this repository</span>
        </p>
        <ul className="note-list">
          {review.data.notes.map((note) => (
            <li key={note}>{note}</li>
          ))}
        </ul>
      </div>
    );
  } else {
    content = <ReviewBody project={project} review={reviewed} others={others} now={now} onSelect={onSelect} />;
  }

  return (
    <article className="card" aria-labelledby="card-title">
      <header className="card-head">
        <p className="card-id mono">{id}</p>
        <h2 id="card-title" className="card-title">
          {summaryOf(reviewed ?? entry)}
        </h2>
        <dl className="facts">
          <div className="fact">
            <dt>Kind</dt>
            <dd className="mono">{reviewed?.kind ?? entry.kind}</dd>
          </div>
          {reviewed !== undefined && (
            <div className="fact">
              <dt>Gap type</dt>
              <dd>{gapLabel(reviewed.gap_type) ?? "None"}</dd>
            </div>
          )}
          <div className="fact">
            <dt>Severity</dt>
            <dd>
              <Badge look={severityLook(reviewed === undefined ? entry.severity : reviewed.severity)} />
            </dd>
          </div>
          <div className="fact">
            <dt>Status</dt>
            <dd>
              <Badge look={statusLook(reviewed?.status ?? entry.status)} />
            </dd>
          </div>
        </dl>
        <div className="decision-bar" role="group" aria-label={`Decide ${id}`}>
          {DECISION_KEYS.map(([kind, key]) => (
            <button
              key={kind}
              type="button"
              className={kind === "accept" ? "button button-primary" : kind === "reject" ? "button button-danger" : "button"}
              aria-keyshortcuts={key}
              aria-disabled={deciding || !ready}
              onClick={() => {
                if (!deciding && ready) {
                  onDecide(kind);
                }
              }}
            >
              <span>{WORDING[kind].verb}</span>
              <kbd aria-hidden="true">{key}</kbd>
            </button>
          ))}
        </div>
      </header>

      <div className="card-body" ref={body} tabIndex={-1}>
        <Section title="Targets">
          {targets.length === 0 ? (
            <p className="muted">No target node named.</p>
          ) : (
            <ul className="target-list">
              {targets.map((target) => (
                <TargetNode key={target} project={project} id={target} />
              ))}
            </ul>
          )}
        </Section>
        {content}
      </div>
    </article>
  );
}
