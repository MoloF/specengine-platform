import { useEffect, useId, useMemo, useRef, useState } from "react";
import { apiErrorOf } from "../api/client";
import { useRecordsRead } from "../api/queries";
import type { FlowNode } from "../api/types";
import { sectionHash } from "../app/routes";
import { DocLink } from "../doc/DocLink";
import { nodeName } from "../graph/layout";
import { useTranslate } from "../i18n";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";
import { EmptyState } from "../ui/EmptyState";
import { ErrorPanel, Skeleton } from "../ui/states";
import { boxWords, holdsText, recordsOf } from "./labels";

/** Entries the dialog shows before "Show all". */
export const RECORDS_SHOWN = 200;

/**
 * Records for the AI (RM-0094 flow-scenario): a 600 px dialog over Flow, its `h2` the
 * title with the count, then what the records are; the scope's read uncut, made only while it is
 * open, listing each record as a document link by title (the modal over the dialog,
 * docs/features/doc-modal.md "Acceptance criteria", DM-10), the first 200, then Show all; the ID-less
 * ones are counted, a Spec tree link. Read only:
 * the AI reads these documents, the canvas leaves them out, and nothing here changes one.
 */
export function FlowRecords({ project, scope, scopeTitle, count, onClose }: { project: string; scope: string; scopeTitle: string; count: number | null; onClose: () => void }) {
  const t = useTranslate();
  const read = useRecordsRead(project, scope, true);
  const [all, setAll] = useState(false);
  const list = useRef<HTMLDivElement>(null);
  const kindId = useId();
  // Show all: focus to the first entry it showed, never the body.
  useEffect(() => {
    if (all) {
      const shown = Array.from(list.current?.querySelectorAll<HTMLElement>("li a") ?? []);
      shown[RECORDS_SHOWN]?.focus();
    }
  }, [all]);
  const listed = useMemo(() => (read.data === undefined ? null : recordsOf(read.data.nodes, scope)), [read.data, scope]);
  const groups = useMemo(() => {
    const shown: { kind: string | null; nodes: FlowNode[] }[] = [];
    let budget = all ? Infinity : RECORDS_SHOWN;
    for (const group of listed?.groups ?? []) {
      const nodes = group.nodes.slice(0, Math.max(0, budget));
      budget -= nodes.length;
      if (nodes.length > 0) {
        shown.push({ ...group, nodes });
      }
    }
    return shown;
  }, [listed, all]);
  return (
    <Dialog title={count === null ? t("flow.records.title") : t("flow.records.open", { count })} onClose={onClose} className="flow-records">
      <p className="dialog-text">
        {t("flow.records.about", { scope: scopeTitle })}
      </p>
      {read.data === undefined && read.error === null && (
        <div aria-busy="true">
          <Skeleton label={t("flow.records.reading")} lines={4} />
        </div>
      )}
      {read.data === undefined && read.error !== null && (
        <ErrorPanel
          title={t("flow.records.failed")}
          message={apiErrorOf(read.error).message}
          retrying={read.isFetching}
          attempt={read.errorUpdateCount}
          onRetry={() => {
            void read.refetch({ cancelRefetch: false });
          }}
        />
      )}
      {read.data !== undefined && read.data.reason !== null && (
        <p className="verbatim" lang="en">
          {read.data.reason}
        </p>
      )}
      {listed !== null && read.data?.reason === null && listed.withId === 0 && listed.noId === 0 && <EmptyState title={t("flow.records.none")} level={3} />}
      {listed !== null && listed.noId > 0 && (
        <p className="flow-records-noid">
          <a href={sectionHash(project, "tree", scope)}>{t("flow.records.noId", { count: listed.noId })}</a>
        </p>
      )}
      {groups.length > 0 && (
        <div ref={list} className="flow-records-list">
          {groups.map((group, index) => (
            // A group named by its heading, no landmark: one kind's records.
            <div key={group.kind ?? ""} className="flow-records-group" role="group" aria-labelledby={`${kindId}-${String(index)}`}>
              <h3 id={`${kindId}-${String(index)}`} className="flow-records-kind">
                {group.kind === null ? t("flow.records.noKind") : <bdi>{group.kind}</bdi>}
              </h3>
              <ul>
                {group.nodes.map((node) => (
                  <li key={`${node.path}:${String(node.line)}`}>
                    <DocLink project={project} docRef={nodeName(node)}>
                      <bdi>{boxWords(node)}</bdi>
                    </DocLink>
                    {node.holds > 0 && <span className="flow-records-holds">{holdsText(node.holds, t)}</span>}
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>
      )}
      <div className="dialog-actions">
        {listed !== null && !all && listed.withId > RECORDS_SHOWN && (
          <Button
            appearance="subtle"
            onClick={() => {
              setAll(true);
            }}
          >
            {t("flow.records.showAll", { count: listed.withId })}
          </Button>
        )}
        <Button onClick={onClose}>{t("ui.close")}</Button>
      </div>
    </Dialog>
  );
}
