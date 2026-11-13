import { useId, useMemo, useRef, useState } from "react";
import { apiErrorOf } from "../api/client";
import { useHeldWorkspaces, type HealthQuery } from "../api/queries";
import type { HealthOrphan, HealthOrphans } from "../api/types";
import { RegionBoundary } from "../app/RegionBoundary";
import { DocLink } from "../doc/DocLink";
import { insideDirections, kindSets, type KindSet } from "../doc/inside";
import { useTranslate } from "../i18n";
import { around, fill, slot } from "../i18n/fill";
import { Button } from "../ui/Button";
import { Lozenge } from "../ui/Lozenge";
import { Notes } from "../ui/served";
import { ReadFailure, Skeleton } from "../ui/states";
import { useFocusLater } from "../ui/useFocusLater";
import { useRetryFocus } from "../ui/useRetryFocus";
import { SHOWN_OPEN } from "./findings";

// Row 8 of What is left, the orphan documents (docs/features/usage-orphans.md "Acceptance criteria",
// UO11): the live documents linked to nothing, as the daemon judged them in `getHealth`, grouped by
// direction, then raw kind, as a document's Inside groups its children. Each row a document link,
// its place, its raw status and its tasks: a task on an orphan means someone plans to bring it in,
// so the row stays. Information only: nothing here holds or gates work (ADR-0012).

/** A group's label: a direction's title in the project's words, a raw kind under `lang="en"`, no kind the chrome's. */
function GroupLabel({ id, set }: { id: string; set: KindSet<HealthOrphan> }) {
  const t = useTranslate();
  if (set.by === "direction") {
    return (
      <h4 id={id} className="health-orphans-label">
        <bdi>{set.title}</bdi>
      </h4>
    );
  }
  return set.kind === null ? (
    <h4 id={id} className="health-orphans-label">
      {t("doc.inside.noKind")}
    </h4>
  ) : (
    <h4 id={id} className="health-orphans-label" lang="en">
      <bdi>{set.kind}</bdi>
    </h4>
  );
}

/** One orphan: the document by its title (else name), `path:line`, its raw status, its tasks; none when the queue was not read. */
function OrphanRow({ project, row }: { project: string; row: HealthOrphan }) {
  const t = useTranslate();
  let tasks = null;
  if (row.tasks !== null) {
    tasks = row.tasks === 0 ? t("health.orphans.noTask") : t("tree.catalog.tasks", { count: row.tasks, open: row.tasks_open ?? 0 });
  }
  return (
    <li className="health-left-row health-orphan" data-orphan={row.path}>
      <DocLink className="health-orphan-link" project={project} docRef={row.name}>
        <bdi>{row.title ?? row.name}</bdi>
      </DocLink>
      <span className="health-orphans-meta">
        <span className="mono">
          {row.path}:{row.line}
        </span>
        {row.status !== null && <Lozenge appearance="neutral" icon="document" label={row.status} lang="en" />}
        {tasks !== null && <span className="health-orphans-tasks">{tasks}</span>}
      </span>
    </li>
  );
}

/** One set's rows, those of the first SHOWN_OPEN among them. */
function OrphanGroup({ project, set, rows }: { project: string; set: KindSet<HealthOrphan>; rows: readonly HealthOrphan[] }) {
  const id = useId();
  return (
    <div className="health-orphans-group" role="group" aria-labelledby={id}>
      <GroupLabel id={id} set={set} />
      <ul className="health-left-list health-orphans-list">
        {rows.map((row) => (
          <OrphanRow key={`${row.path}:${String(row.line)}`} project={project} row={row} />
        ))}
      </ul>
    </div>
  );
}

/** The sets cut to their first `limit` rows in set order, a set left with none dropped. */
function firstRows(sets: readonly KindSet<HealthOrphan>[], limit: number): { set: KindSet<HealthOrphan>; rows: HealthOrphan[] }[] {
  const cut: { set: KindSet<HealthOrphan>; rows: HealthOrphan[] }[] = [];
  let left = limit;
  for (const set of sets) {
    const rows = set.entries.slice(0, left);
    left -= rows.length;
    if (rows.length > 0) {
      cut.push({ set, rows });
    }
  }
  return cut;
}

/**
 * The measured row: how many over how many live documents and what that means, then the orphans by
 * direction and kind; the first SHOWN_OPEN drawn, the rest not until "Show all <n>", which hands the
 * focus to the first row it adds. None: said as the rule, never as nothing left.
 */
function Orphans({ project, orphans }: { project: string; orphans: HealthOrphans }) {
  const t = useTranslate();
  const [all, setAll] = useState(false);
  const groups = useRef<HTMLDivElement>(null);
  const focusLater = useFocusLater();
  const held = useHeldWorkspaces(project);
  const modules = held.data?.modules;
  const sets = useMemo(() => kindSets(orphans.rows, insideDirections(modules ?? [])), [orphans.rows, modules]);
  const notes = <Notes notes={orphans.notes} label={t("health.orphans.notes")} />;
  const over = t("health.orphans.over", { count: orphans.live });
  const count = orphans.rows.length;
  if (count === 0) {
    return (
      <>
        {notes}
        <p className="health-left-total">{fill(t("health.orphans.none", { none: slot("none") }), { none: <span className="health-value health-measured">{t("health.amendments.noneWord")}</span> })}</p>
        <p className="health-orphans-scope">{t("health.orphans.measuredOver", { over })}</p>
      </>
    );
  }
  const [before, after] = around(t("health.orphans.total", { count, over }), String(count));
  const shown = firstRows(sets, all ? count : SHOWN_OPEN);
  return (
    <>
      {notes}
      <p className="health-left-total">
        {before}
        <strong className="health-value health-measured">{count}</strong>
        {after}
      </p>
      <p className="health-orphans-scope">{t("health.orphans.meaning")}</p>
      <div ref={groups} className="health-orphans-groups" role="group" aria-label={t("health.orphans.listLabel")}>
        {shown.map(({ set, rows }) => (
          <OrphanGroup key={set.by === "direction" ? `direction:${set.name}` : `kind:${set.kind ?? ""}`} project={project} set={set} rows={rows} />
        ))}
      </div>
      {!all && count > SHOWN_OPEN && (
        <Button
          onClick={() => {
            setAll(true);
            focusLater(() => groups.current?.querySelectorAll<HTMLElement>(".health-orphan-link")[SHOWN_OPEN]);
          }}
        >
          {t("health.amendments.showAll", { count })}
        </Button>
      )}
    </>
  );
}

/**
 * Row 8 from the health read, with its own boundary, after Drift: being measured (a skeleton, no
 * digit), could not be measured (the daemon's words, shown, not spoken: row 3 speaks them, and a
 * Retry reading `getHealth` alone), measured, zero too. `failure` and `busy` are What is left's.
 */
export function OrphansPart({ project, health, failure, busy }: { project: string; health: HealthQuery; failure: Error | null; busy: boolean }) {
  const t = useTranslate();
  const heading = useRef<HTMLHeadingElement>(null);
  const retried = useRetryFocus(health.data, () => heading.current);

  let body;
  if (health.data === undefined) {
    body =
      failure === null ? (
        <Skeleton label={t("health.orphans.measuring", { project })} lines={3} />
      ) : (
        <ReadFailure
          title={t("health.orphans.failed")}
          failure={failure}
          retrying={health.isFetching}
          attempt={health.errorUpdateCount}
          announce={false}
          onRetry={() => {
            retried();
            void health.refetch({ cancelRefetch: false });
          }}
        />
      );
  } else {
    body = (
      <>
        <Orphans project={project} orphans={health.data.orphans} />
        {health.error !== null && !health.isFetching && (
          <p className="note">{fill(t("health.orphans.failedAgain", { message: slot("message") }), { message: <span lang="en">{apiErrorOf(health.error).message}</span> })}</p>
        )}
      </>
    );
  }

  return (
    <div className="health-part health-orphans" data-row="orphans" aria-busy={busy}>
      <h3 className="health-part-title" ref={heading} tabIndex={-1}>
        {t("health.orphans.title")}
      </h3>
      <RegionBoundary name={t("health.orphans.boundary")}>{body}</RegionBoundary>
    </div>
  );
}
