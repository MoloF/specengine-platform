import { useId, useMemo, useRef, useState } from "react";
import { apiErrorOf } from "../api/client";
import { useCatalog } from "../api/queries";
import type { CatalogChange, CatalogRow } from "../api/types";
import { DocLink } from "../doc/DocLink";
import { rollupLook } from "../flow/labels";
import { useLanguage, useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { Badge } from "../ui/Badge";
import { Button } from "../ui/Button";
import { EmptyState } from "../ui/EmptyState";
import { Icon } from "../ui/Icon";
import { ErrorPanel, Skeleton } from "../ui/states";
import { formatUtc } from "../ui/time";
import { useFocusLater } from "../ui/useFocusLater";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import {
  ariaSort,
  CATALOG_COLUMNS,
  CATALOG_FILTERS,
  CATALOG_SHOWN,
  filteredRows,
  pressed,
  sortedRows,
  tasksText,
  titleOf,
  versionText,
  type CatalogColumn,
  type CatalogFilter,
  type CatalogSort,
} from "./catalog";
import { placeKey, rowName } from "./rows";

// A kinds direction as a table of its nodes, in the Spec tree's node column while no node is open
// (docs/features/direction-catalog.md "Acceptance criteria", DC-6, DC-10), a chunk of its own: the
// daemon's rows, each node's title opening the document modal, its version, state, open items,
// tasks and last change, the first 50 then Show more. A press on a header orders the served rows;
// before one they stand as served. Read only: it moves, refuses and gates nothing (ADR-0012). Used by
// (docs/features/usage-orphans.md "Acceptance criteria", UO11): the served count, its mentions, an
// orphan's neutral badge; No declared use and Not cited keep the rows the served counts say, the
// caption saying how many of all. A count of 0 with mentions shows them beside it at full weight.

/** A header: its column's name on a subtle compact button, the order it holds said by `aria-sort`. */
function SortHead({ column, sort, onPress }: { column: CatalogColumn; sort: CatalogSort | null; onPress: (column: CatalogColumn) => void }) {
  const t = useTranslate();
  const order = ariaSort(sort, column);
  return (
    <th scope="col" aria-sort={order}>
      <Button
        appearance="subtle"
        spacing="compact"
        onClick={() => {
          onPress(column);
        }}
      >
        <span>{t(`tree.catalog.column.${column}`)}</span>
        {order !== undefined && <Icon name={order === "descending" ? "cardDown" : "cardUp"} />}
      </Button>
    </th>
  );
}

/** A row's last change: its time and who, by name alone; a section's marked as its file's; none read, Not committed. */
function LastChange({ change }: { change: CatalogChange | null }) {
  const t = useTranslate();
  if (change === null) {
    return <span className="catalog-none">{t("tree.catalog.noChange")}</span>;
  }
  return (
    <>
      {fill(t("tree.catalog.changed", { at: slot("at"), name: slot("name") }), {
        at: (
          <time className="catalog-time" dateTime={change.at}>
            {formatUtc(change.at, t)}
          </time>
        ),
        name: <bdi>{change.by_name}</bdi>,
      })}
      {change.of_file && (
        <>
          {" "}
          <span className="catalog-of-file">{t("tree.catalog.ofFile")}</span>
        </>
      )}
    </>
  );
}

/**
 * What uses a row, as served: the count; its mentions, beside a 0 at full weight ("0 · 6 mentions",
 * so a record cited only through mentions never reads as unused), else small after a "+"; then a
 * neutral Orphan badge where the core flags one.
 */
function UsedCell({ row }: { row: CatalogRow }) {
  const t = useTranslate();
  const mentions = t("tree.catalog.mentions", { count: row.mentioned_by });
  return (
    <>
      {row.used_by}
      {row.mentioned_by > 0 &&
        (row.used_by === 0 ? (
          <>
            {" · "}
            <span className="catalog-used-cited">{mentions}</span>
          </>
        ) : (
          <>
            {" "}
            <span className="catalog-used-mentions">+ {mentions}</span>
          </>
        ))}
      {row.orphan === true && (
        <>
          {" "}
          <Badge look={{ label: t("tree.catalog.orphan"), tone: "usage-orphan", icon: "linkDangling" }} />
        </>
      )}
    </>
  );
}

/** One served row: no OID, email or proposal of it is drawn. */
function CatalogLine({ project, row }: { project: string; row: CatalogRow }) {
  const t = useTranslate();
  const name = rowName(row);
  return (
    <tr data-row={name}>
      <th scope="row">
        <DocLink project={project} docRef={name} className="catalog-link">
          <bdi>{titleOf(row)}</bdi>
        </DocLink>
        {row.title !== null && row.id !== null && <span className="catalog-id mono">{row.id}</span>}
      </th>
      <td className="catalog-version">{versionText(row, t)}</td>
      <td className="catalog-state">
        <Badge look={rollupLook(row.state, t)} />
      </td>
      <td className="catalog-count">{row.open_items}</td>
      <td className="catalog-count catalog-tasks">{tasksText(row, t)}</td>
      <td className="catalog-count catalog-used">
        <UsedCell row={row} />
      </td>
      <td className="catalog-changed">
        <LastChange change={row.last_change} />
      </td>
    </tr>
  );
}

/**
 * The table: the first 50 of the current order, then Show more, focus moving to the first row it
 * shows. Used by 0 pressed keeps the rows nothing uses, the caption "K of N"; none left, an empty state.
 */
function CatalogTable({ project, direction, rows }: { project: string; direction: string; rows: readonly CatalogRow[] }) {
  const t = useTranslate();
  const { language } = useLanguage();
  const focusLater = useFocusLater();
  const captionId = useId();
  const body = useRef<HTMLTableSectionElement>(null);
  const collator = useMemo(() => new Intl.Collator(language, { numeric: true }), [language]);
  const [sort, setSort] = useState<CatalogSort | null>(null);
  const [shown, setShown] = useState(CATALOG_SHOWN);
  const [filter, setFilter] = useState<CatalogFilter | null>(null);
  const kept = useMemo(() => filteredRows(rows, filter), [rows, filter]);
  const ordered = useMemo(() => sortedRows(kept, sort, collator), [kept, sort, collator]);
  const visible = ordered.slice(0, shown);
  const rest = ordered.length - visible.length;
  const ofFile = visible.some((row) => row.last_change?.of_file === true);

  function more() {
    const first = visible.length;
    setShown(first + CATALOG_SHOWN);
    focusLater(() => body.current?.rows.item(first)?.querySelector("a"));
  }

  const caption =
    filter === null
      ? t("tree.catalog.caption", { count: rows.length, direction: slot("direction") })
      : t("tree.catalog.captionOf", { count: rows.length, shown: kept.length, direction: slot("direction") });
  // One filter at a time: a press takes it, a press on the one held lets it go.
  const filters = (
    <div className="catalog-used-filter" role="group" aria-label={t("tree.catalog.filterLabel")}>
      {CATALOG_FILTERS.map((each) => (
        <Button
          key={each}
          spacing="compact"
          aria-pressed={filter === each}
          onClick={() => {
            setFilter(filter === each ? null : each);
          }}
        >
          {t(`tree.catalog.filter.${each}`)}
        </Button>
      ))}
    </div>
  );
  if (kept.length === 0) {
    return (
      <>
        {filters}
        <EmptyState level={2} title={t("tree.catalog.filterNone")} />
      </>
    );
  }

  return (
    <>
      {filters}
      <div className="catalog-table-wrap" role="region" aria-labelledby={captionId} tabIndex={0}>
        <table className="catalog-table">
          <caption id={captionId}>{fill(caption, { direction: <bdi>{direction}</bdi> })}</caption>
          <thead>
            <tr>
              {CATALOG_COLUMNS.map((column) => (
                <SortHead
                  key={column}
                  column={column}
                  sort={sort}
                  onPress={(next) => {
                    setSort(pressed(sort, next));
                  }}
                />
              ))}
            </tr>
          </thead>
          <tbody ref={body}>
            {visible.map((row) => (
              <CatalogLine key={placeKey(row)} project={project} row={row} />
            ))}
          </tbody>
        </table>
      </div>
      {ofFile && <p className="catalog-hint">{t("tree.catalog.ofFileHint")}</p>}
      {rest > 0 && (
        <Button onClick={more}>
          {t("tree.catalog.more", { count: Math.min(rest, CATALOG_SHOWN) })}
        </Button>
      )}
    </>
  );
}

/**
 * The direction's table, read while it is on screen: a skeleton, then the rows, none an empty state;
 * a failure in the daemon's words with Retry, the last table kept below it; the daemon's notes last.
 * `kinds` as the direction serves them; `direction` its title.
 */
export function DirectionCatalog({ project, kinds, direction, archive }: { project: string; kinds: readonly string[]; direction: string; archive: boolean }) {
  const t = useTranslate();
  const catalog = useCatalog(project, kinds, archive);
  const failure = useRetainedFailure(catalog.error, catalog.isFetching);
  const region = useRef<HTMLDivElement>(null);
  const retried = useRetryFocus(catalog.data, () => region.current?.querySelector<HTMLElement>(".catalog-table-wrap") ?? region.current);
  const data = catalog.data;
  return (
    <div ref={region} className="catalog" tabIndex={-1} aria-busy={catalog.isFetching}>
      {failure !== null && (
        <ErrorPanel
          title={t("tree.catalog.failed", { direction })}
          message={apiErrorOf(failure).message}
          retrying={catalog.isFetching}
          attempt={catalog.errorUpdateCount}
          onRetry={() => {
            retried();
            void catalog.refetch({ cancelRefetch: false });
          }}
        />
      )}
      {data === undefined ? (
        failure === null && <Skeleton label={t("tree.catalog.loading", { direction })} lines={6} />
      ) : data.rows.length === 0 ? (
        <EmptyState level={2} title={t("tree.catalog.empty")} description={<p>{t("tree.catalog.emptyText")}</p>} />
      ) : (
        <CatalogTable project={project} direction={direction} rows={data.rows} />
      )}
      {data !== undefined && data.notes.length > 0 && (
        <ul className="note-list" aria-label={t("tree.catalog.notes")}>
          {data.notes.map((note) => (
            <li key={note} lang="en">
              {note}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
