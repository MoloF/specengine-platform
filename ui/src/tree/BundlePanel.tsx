import { useId, useRef, useState, type MouseEvent, type SubmitEvent } from "react";
import { apiErrorOf } from "../api/client";
import { useBundle } from "../api/queries";
import type { BundleBindingRow, BundleItem, BundleLayers, BundleView, TailEntry } from "../api/types";
import { sectionHash } from "../app/routes";
import { Badge } from "../ui/Badge";
import { Button } from "../ui/Button";
import { EmptyState } from "../ui/EmptyState";
import { ErrorPanel, Skeleton } from "../ui/states";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { BindingLine } from "./BindingLine";
import { directionLabel, formLabel, isCodeLayer, layerHeading, layerHeadings, linkStateLook } from "./labels";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";

const DIGITS = /^\d*$/;
/** The bundle's hash as the CLI names it, shown as code. */
const BUNDLE_HASH = "bundle_hash";
/** The largest budget the daemon takes (`u32::MAX`, `docs/canon/spec-cli-bundle.md` "Command"). */
const MAX_BUDGET = 4294967295;

function ItemRow({ project, item, onFollow }: { project: string; item: BundleItem; onFollow: (event: MouseEvent<HTMLAnchorElement>) => void }) {
  const t = useTranslate();
  const answer = item.working_answer;
  return (
    <li className="bundle-item">
      <p className="bundle-item-head">
        <a className="mono" href={sectionHash(project, "tree", item.name)} onClick={onFollow}>
          {item.name}
        </a>
        <span className="kind-tag">{item.kind ?? "-"}</span>
        <span>{item.title ?? "-"}</span>
      </p>
      <p className="bundle-item-meta">
        <span className="mono">
          {item.path}:{item.line}
        </span>
        <span>{formLabel(item.form, t)}</span>
        <span>{t("tree.binding.tokens", { count: item.tokens_est })}</span>
        {item.status !== null && (
          <span className="status-tag">
            <span className="sr-only">{t("tree.row.status")}</span>
            {item.status}
          </span>
        )}
        {item.archived && <span className="plain-tag">{t("tree.row.archived")}</span>}
      </p>
      {item.via !== null && item.via.length > 0 && (
        <p className="bundle-item-meta">
          {fill(t("tree.bundle.via", { links: slot("links") }), {
            links: item.via.map((via, index) => (
              <span key={`${via.type}-${via.direction}`}>
                {index > 0 && ", "}
                <span className="mono">{via.type}</span> {directionLabel(via.direction, t)}
              </span>
            )),
          })}
        </p>
      )}
      {answer !== null && (
        <p className="bundle-item-meta">
          <span>{t("tree.bundle.workingAnswer")}</span>
          <span className="mono">{answer.name ?? answer.written}</span>
          <span className="mono">
            {answer.path}:{answer.line}
          </span>
          <Badge name={t("tree.links.state")} look={linkStateLook(answer.state, t)} />
        </p>
      )}
    </li>
  );
}

/**
 * What tells two rows apart: their place in the layer, then verify's key and, for a keyless row,
 * its state and category (`docs/canon/spec-verify.md` "Rows"). A listed row has neither
 * (`docs/canon/code-reads.md` "Degrade"), so two keyless ones on one node, relation and file
 * differ by the index alone, as the tail's entries do.
 */
function rowKey(row: BundleBindingRow, index: number): string {
  return [String(index), row.node, row.relation, row.file, row.kind, row.symbol, row.item_kind, row.sync, row.category]
    .map((part) => part ?? "")
    .join("\u0000");
}

/** A layer's entries: nodes as items linked to the tree, code rows as lines. */
function LayerList({
  project,
  layers,
  layer,
  onFollow,
}: {
  project: string;
  layers: BundleLayers;
  layer: keyof BundleLayers;
  onFollow: (event: MouseEvent<HTMLAnchorElement>) => void;
}) {
  if (isCodeLayer(layer)) {
    return (
      <ul className="bundle-items">
        {layers[layer].map((row, index) => (
          <BindingLine key={rowKey(row, index)} row={row} tokens={row.tokens_est} noId="-" />
        ))}
      </ul>
    );
  }
  return (
    <ul className="bundle-items">
      {layers[layer].map((item) => (
        <ItemRow key={`${item.path}:${String(item.line)}`} project={project} item={item} onFollow={onFollow} />
      ))}
    </ul>
  );
}

/** A left-out entry: a node linked to the tree; a code row named, with its place, never linked. */
function TailLine({ project, entry, onFollow }: { project: string; entry: TailEntry; onFollow: (event: MouseEvent<HTMLAnchorElement>) => void }) {
  const t = useTranslate();
  if (isCodeLayer(entry.layer)) {
    return (
      <li>
        <span className="mono">{entry.name}</span> <span>{entry.title ?? "-"}</span>{" "}
        <span className="muted">
          | <span className="mono">{entry.path}:{entry.line}</span> | {t("tree.binding.tokens", { count: entry.tokens_est })} | {layerHeading(entry.layer, t)}
        </span>
      </li>
    );
  }
  return (
    <li>
      <a className="mono" href={sectionHash(project, "tree", entry.name)} onClick={onFollow}>
        {entry.name}
      </a>{" "}
      <span>{entry.title ?? "-"}</span>{" "}
      <span className="muted">
        | {t("tree.binding.tokens", { count: entry.tokens_est })} | {layerHeading(entry.layer, t)}
      </span>
    </li>
  );
}

function BundleBody({ project, bundle, onFollow }: { project: string; bundle: BundleView; onFollow: (event: MouseEvent<HTMLAnchorElement>) => void }) {
  const t = useTranslate();
  if (bundle.reason !== null) {
    return (
      <EmptyState
        level={2}
        title={t("tree.bundle.none")}
        description={
          <p className="verbatim" lang="en">
            {bundle.reason}
          </p>
        }
      />
    );
  }
  const tail = bundle.tail ?? [];
  const more = bundle.more ?? 0;
  const layers = bundle.layers;
  return (
    <>
      <p className="bundle-summary">
        {t("tree.bundle.summary", {
          tokens: bundle.tokens ?? "-",
          budget: bundle.budget ?? "-",
          chars: bundle.chars ?? "-",
          bytes: bundle.bytes ?? "-",
          left: tail.length + more,
        })}
      </p>
      {bundle.bundle_hash !== null && (
        <p className="bundle-hash">
          <span className="muted">{BUNDLE_HASH}</span> <span className="mono">{bundle.bundle_hash}</span>
        </p>
      )}
      {bundle.notes.length > 0 && (
        <ul className="note-list" aria-label={t("tree.bundle.notes")}>
          {bundle.notes.map((note) => (
            <li key={note} lang="en">
              {note}
            </li>
          ))}
        </ul>
      )}
      {layers !== null &&
        layerHeadings(t).filter(([key]) => layers[key].length > 0).map(([key, heading]) => (
          <section key={key} className="bundle-layer">
            <h2 className="card-section-title">{heading}</h2>
            <LayerList project={project} layers={layers} layer={key} onFollow={onFollow} />
          </section>
        ))}
      {(tail.length > 0 || more > 0) && (
        <section className="bundle-layer">
          <h2 className="card-section-title">{t("tree.bundle.notIncluded")}</h2>
          <ul className="bundle-tail">
            {tail.map((entry, index) => (
              // A code row's place may repeat (two relations, one symbol): the index keeps keys distinct.
              <TailLine key={`${String(index)}:${entry.path}:${String(entry.line)}`} project={project} entry={entry} onFollow={onFollow} />
            ))}
            {more > 0 && <li className="muted">{t("tree.bundle.more", { count: more })}</li>}
          </ul>
        </section>
      )}
      {bundle.body !== null && (
        <details className="bundle-body">
          <summary>{t("tree.bundle.body")}</summary>
          <pre className="node-text">{bundle.body}</pre>
        </details>
      )}
    </>
  );
}

/**
 * The Bundle tab: the context an agent gets for this node. Read when the tab first opens, with the
 * project's default budget; then once per "Get bundle". The budget field takes digits only; a
 * refusal (a budget under the minimum) is shown in the daemon's words and the typed value stays.
 */
export function BundlePanel({ project, nodeRef, onFollow }: { project: string; nodeRef: string; onFollow: (event: MouseEvent<HTMLAnchorElement>) => void }) {
  const t = useTranslate();
  const fieldId = useId();
  const [typed, setTyped] = useState("");
  const [budget, setBudget] = useState<number | null>(null);
  /** What the budget field says is wrong: not digits, or more than the daemon takes. */
  const [fieldError, setFieldError] = useState<"digits" | "tooLarge" | null>(null);
  /** Retry was pressed: its panel, and the Retry that has focus, stay up until the answer. */
  const [retrying, setRetrying] = useState(false);
  const query = useBundle(project, budget === null ? { node_ids: [nodeRef] } : { node_ids: [nodeRef], budget }, true);
  const failure = useRetainedFailure(query.error, retrying && query.isFetching);
  const result = useRef<HTMLDivElement>(null);
  const retried = useRetryFocus(query.data, () => result.current);

  function submit(event: SubmitEvent<HTMLFormElement>) {
    event.preventDefault();
    const value = typed.trim();
    if (!DIGITS.test(value)) {
      setFieldError("digits");
      return;
    }
    const next = value === "" ? null : Number(value);
    if (next !== null && next > MAX_BUDGET) {
      setFieldError("tooLarge");
      return;
    }
    setFieldError(null);
    setRetrying(false);
    if (next === budget) {
      void query.refetch({ cancelRefetch: false });
    } else {
      setBudget(next);
    }
  }

  const errorId = `${fieldId}-error`;
  let content;
  if (failure !== null) {
    content = (
      <ErrorPanel
        title={t("tree.bundle.failed")}
        message={apiErrorOf(failure).message}
        retrying={query.isFetching}
        attempt={query.errorUpdateCount}
        onRetry={() => {
          retried();
          setRetrying(true);
          void query.refetch({ cancelRefetch: false });
        }}
      />
    );
  } else if (query.data === undefined) {
    content = (
      <div aria-busy="true">
        <Skeleton label={t("tree.bundle.loading", { ref: nodeRef })} lines={5} />
      </div>
    );
  } else {
    content = (
      <div ref={result} tabIndex={-1} className="bundle-result" aria-busy={query.isFetching}>
        <BundleBody project={project} bundle={query.data} onFollow={onFollow} />
      </div>
    );
  }

  return (
    <div className="bundle-panel">
      <p className="muted">{t("tree.bundle.about", { ref: nodeRef })}</p>
      <form className="bundle-form" onSubmit={submit} noValidate>
        <div className="bundle-field">
          <label className="field-label" htmlFor={fieldId}>
            {t("tree.bundle.budget")}
          </label>
          <input
            id={fieldId}
            className="field"
            inputMode="numeric"
            autoComplete="off"
            spellCheck={false}
            placeholder={t("tree.bundle.default")}
            value={typed}
            aria-invalid={fieldError !== null}
            aria-describedby={fieldError === null ? undefined : errorId}
            onChange={(event) => {
              setTyped(event.target.value);
              if (fieldError !== null && DIGITS.test(event.target.value.trim())) {
                setFieldError(null);
              }
            }}
          />
        </div>
        <Button type="submit">{t("tree.bundle.get")}</Button>
        {fieldError !== null && (
          <p id={errorId} className="field-error" role="alert">
            {fieldError === "digits" ? t("tree.bundle.digits") : t("tree.bundle.tooLarge", { max: MAX_BUDGET })}
          </p>
        )}
      </form>
      {content}
    </div>
  );
}
