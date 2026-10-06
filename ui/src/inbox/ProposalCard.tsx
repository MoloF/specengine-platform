import { useEffect, useRef } from "react";
import { apiErrorOf } from "../api/client";
import { useNode } from "../api/queries";
import type { Proposal } from "../api/types";
import { sectionHash } from "../app/routes";
import { Badge } from "../ui/Badge";
import { DiffView } from "../ui/DiffView";
import { focusIsLost } from "../ui/focus";
import { Icon } from "../ui/Icon";
import { ErrorPanel, Section, Skeleton } from "../ui/states";
import { formatAge, formatUtc } from "../ui/time";
import { useRetainedFailure } from "../ui/useRetainedFailure";
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

export function ProposalCard({
  project,
  proposal,
  others,
  now,
  deciding,
  onDecide,
  onSelect,
}: {
  project: string;
  proposal: Proposal;
  /** Other proposals in the queue on the same nodes: information only, nothing waits on them. */
  others: readonly Proposal[];
  now: number;
  /** A decision is being sent: the buttons stay focusable but do nothing. */
  deciding: boolean;
  onDecide: (kind: DecisionKind) => void;
  onSelect: (id: string) => void;
}) {
  const targets = targetsOf(proposal);
  const preview = previewLabel(proposal.preview);
  const author = proposal.author;
  return (
    <article className="card" aria-labelledby="card-title">
      <header className="card-head">
        <p className="card-id mono">{proposal.id}</p>
        <h2 id="card-title" className="card-title">
          {summaryOf(proposal)}
        </h2>
        <dl className="facts">
          <div className="fact">
            <dt>Kind</dt>
            <dd className="mono">{proposal.kind}</dd>
          </div>
          <div className="fact">
            <dt>Gap type</dt>
            <dd>{gapLabel(proposal.gap_type) ?? "None"}</dd>
          </div>
          <div className="fact">
            <dt>Severity</dt>
            <dd>
              <Badge look={severityLook(proposal.severity)} />
            </dd>
          </div>
          <div className="fact">
            <dt>Task</dt>
            <dd className="mono">
              {proposal.task_id === null ? "No task" : <a href={sectionHash(project, "tasks", proposal.task_id)}>{proposal.task_id}</a>}
            </dd>
          </div>
          <div className="fact">
            <dt>Status</dt>
            <dd>
              <Badge look={statusLook(proposal.status)} />
            </dd>
          </div>
        </dl>
        <div className="decision-bar" role="group" aria-label={`Decide ${proposal.id}`}>
          {DECISION_KEYS.map(([kind, key]) => (
            <button
              key={kind}
              type="button"
              className={kind === "accept" ? "button button-primary" : kind === "reject" ? "button button-danger" : "button"}
              aria-keyshortcuts={key}
              aria-disabled={deciding}
              onClick={() => {
                if (!deciding) {
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

      <div className="card-body">
        <Section title="Targets">
          {targets.length === 0 ? (
            <p className="muted">No target node named.</p>
          ) : (
            <ul className="target-list">
              {targets.map((id) => (
                <TargetNode key={id} project={project} id={id} />
              ))}
            </ul>
          )}
        </Section>

        {proposal.evidence.length > 0 && (
          <Section title="Evidence">
            <ul className="evidence-list">
              {proposal.evidence.map((item, index) => (
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
          {proposal.options.length === 0 ? (
            <p className="muted">No options attached.</p>
          ) : (
            <ol className="option-list">
              {proposal.options.map((choice, index) => {
                const recommended = index === proposal.recommendation;
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

        {proposal.working_answer !== null && (
          <Section title="Working answer">
            <p className="prose">{proposal.working_answer}</p>
          </Section>
        )}

        {proposal.rationale !== null && proposal.rationale !== proposal.summary && (
          <Section title="Rationale">
            <p className="prose">{proposal.rationale}</p>
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
            <dt>Raised</dt>
            <dd>
              <time dateTime={proposal.created_at}>
                {formatAge(proposal.created_at, now)} ({formatUtc(proposal.created_at)})
              </time>
            </dd>
            {proposal.branch !== null && (
              <>
                <dt>Branch</dt>
                <dd className="mono">{proposal.branch}</dd>
              </>
            )}
            {proposal.worktree !== null && (
              <>
                <dt>Worktree</dt>
                <dd className="mono">{proposal.worktree}</dd>
              </>
            )}
            {proposal.decision_note !== null && (
              <>
                <dt>Last decision note</dt>
                <dd>{proposal.decision_note}</dd>
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
          <DiffView diff={proposal.diff} />
          {preview !== null && <p className="muted">Apply preview: {preview}.</p>}
          {proposal.conflict !== null && (
            <>
              <p className="target-label">Conflict at apply</p>
              <pre className="node-text">{proposal.conflict}</pre>
            </>
          )}
        </Section>

        {proposal.diagnostics.length > 0 && (
          <Section title="Check findings the change introduces">
            <ul className="finding-list">
              {proposal.diagnostics.map((finding, index) => (
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

        {proposal.notes.length > 0 && (
          <Section title="Notes">
            <ul className="note-list">
              {proposal.notes.map((note) => (
                <li key={note}>{note}</li>
              ))}
            </ul>
          </Section>
        )}
      </div>
    </article>
  );
}
