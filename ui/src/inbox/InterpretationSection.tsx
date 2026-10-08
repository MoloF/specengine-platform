import { useRef } from "react";
import { apiErrorOf } from "../api/client";
import { useNode } from "../api/queries";
import type { Interpretation } from "../api/types";
import { sectionHash } from "../app/routes";
import { shownSlot, shownText } from "../round/text";
import { ErrorPanel, Section } from "../ui/states";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";

/**
 * The accepted decision an interpretation names (its document's `id:`), read now: linked to its
 * node in the spec tree, with its title, when the read finds it; the read's reason, unlinked, when
 * it finds none; while reading, the ID alone in a busy block (the value is known, so no skeleton
 * hides it); a failed read in the daemon's words with Retry, after which focus goes to the ID
 * instead of the page's body.
 */
function DecisionRef({ project, decision }: { project: string; decision: string }) {
  const query = useNode(project, decision);
  const failure = useRetainedFailure(query.error, query.isFetching);
  const name = useRef<HTMLElement | null>(null);
  const keep = (element: HTMLElement | null) => {
    name.current = element;
  };
  const retried = useRetryFocus(query.data, () => name.current);
  const view = query.data;
  const node = view?.nodes[0];
  // The ID as stored, a one-line slot: controls written `\u{..}`, isolated from its neighbours.
  const id = <bdi>{shownSlot(decision)}</bdi>;
  if (view === undefined) {
    return (
      <div className="decision-ref" aria-busy={failure === null}>
        <p>
          <span ref={keep} className="mono" tabIndex={-1}>
            {id}
          </span>
        </p>
        {failure !== null && (
          <ErrorPanel
            title={`The decision ${shownSlot(decision)} could not be read`}
            message={apiErrorOf(failure).message}
            retrying={query.isFetching}
            attempt={query.errorUpdateCount}
            onRetry={() => {
              retried();
              void query.refetch({ cancelRefetch: false });
            }}
          />
        )}
      </div>
    );
  }
  if (node === undefined) {
    return (
      <div className="decision-ref">
        <p>
          <span ref={keep} className="mono" tabIndex={-1}>
            {id}
          </span>
        </p>
        {/* The daemon's reason quotes the stored ID: its controls written `\u{..}` as the Questions screen writes a note, its direction its own. */}
        <p className="muted" dir="auto">
          {view.reason === null ? "No node answers this ID." : shownSlot(view.reason)}
        </p>
      </div>
    );
  }
  return (
    <div className="decision-ref">
      <p>
        <a ref={keep} href={sectionHash(project, "tree", decision)}>
          <span className="mono">{id}</span>
          {node.title !== null && (
            <>
              {": "}
              <bdi>{node.title}</bdi>
            </>
          )}
        </a>
      </p>
    </div>
  );
}

/**
 * What an interpretation carries (`docs/features/kind-interpretation.md` "Data"): the decision,
 * then how it was applied and why interpretation was needed, each the agent's text verbatim, never
 * markdown: line feeds and tabs kept, every other control written `\u{..}`, each line taking its
 * own direction. Shown for any proposal whose document holds one, whatever its kind.
 */
export function InterpretationSection({ project, interpretation }: { project: string; interpretation: Interpretation }) {
  return (
    <Section title="Interpretation">
      <dl className="interpretation">
        <div className="interpretation-part">
          <dt>Decision</dt>
          <dd>
            <DecisionRef key={interpretation.decision} project={project} decision={interpretation.decision} />
          </dd>
        </div>
        <div className="interpretation-part">
          <dt>How the decision was applied</dt>
          <dd>
            <p className="agent-text" dir="auto">
              {shownText(interpretation.how)}
            </p>
          </dd>
        </div>
        <div className="interpretation-part">
          <dt>Why interpretation was needed</dt>
          <dd>
            <p className="agent-text" dir="auto">
              {shownText(interpretation.why)}
            </p>
          </dd>
        </div>
      </dl>
    </Section>
  );
}
