import { Fragment, useEffect, useId, useMemo, useRef, useState, type ReactNode, type Ref } from "react";
import { apiErrorOf } from "../api/client";
import { useLibrary } from "../api/queries";
import type { LibraryChange, LibraryDecision, LibraryDocument, LibraryRow, WorkspaceModule } from "../api/types";
import { aboutOf } from "../app/sections";
import { DocLink } from "../doc/DocLink";
import { useLanguage, useTranslate, type Translate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { UsedCell } from "../tree/UsedCell";
import { Button } from "../ui/Button";
import { EmptyState } from "../ui/EmptyState";
import { Icon } from "../ui/Icon";
import { Lozenge } from "../ui/Lozenge";
import { ErrorPanel, Skeleton } from "../ui/states";
import { formatUtc } from "../ui/time";
import { useFocusLater } from "../ui/useFocusLater";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { ALL_MODULES } from "../workspace/choice";
import {
  ariaSort,
  atAll,
  decisionCounts,
  decisionCountText,
  decisionTitle,
  FACET_ORDER,
  facetChoices,
  groupLabel,
  groupsOf,
  isFiltered,
  isSorted,
  keptRows,
  LIBRARY_COLUMNS,
  LIBRARY_SHOWN,
  moduleChoices,
  NO_FILTERS,
  pressed,
  sortedRows,
  startingFilters,
  titleOf,
  toggleCount,
  toggled,
  type FacetValue,
  type LibraryFilters,
  type LibrarySort,
  type MultiFacet,
  type SortedColumn,
} from "./library";

// Documents, `#/<p>/documents` (docs/features/doc-library.md "Acceptance criteria", DL18, DL19), a chunk of
// its own: every document the daemon's library serves, as a table filtered by facets over that one
// answer. Read only: a title opens the document modal, a Decisions cell opens the decisions whose
// `canon:` lands on the row; nothing here writes, gates or moves anything (ADR-0004, ADR-0012). The
// Module facet is the module bar's stored choice, read and written through the Shell; every other
// choice is kept per project for the page's life. Served values are shown as written, in any script.

/** A project's choices for this page's life: the facets (null until a person picks, so the start follows the rows read), Include archive as last answered, the order. */
export interface DocumentsState {
  filters: LibraryFilters | null;
  archive: boolean;
  sort: LibrarySort | null;
}

/** Where the Shell keeps each project's DocumentsState for the page's life. */
export interface DocumentsMemory {
  recall: (project: string) => DocumentsState | undefined;
  remember: (project: string, state: DocumentsState) => void;
}

/** A served value as written, or a dash where none is. */
function Written({ value }: { value: string | number | null }) {
  return value === null ? <span className="documents-none">-</span> : <bdi>{value}</bdi>;
}

/** A facet value's words: the served value as written, else the facet's "No …". */
function valueLabel(facet: MultiFacet, value: FacetValue, t: Translate): string {
  return value === null ? t(`documents.none.${facet}`) : value;
}

/** A pressable value with its count: the chip of Activity's filters, pressed by a tick and a solid border, never colour alone. */
function Chip({ pressed: isPressed, onPress, count, children }: { pressed: boolean; onPress: () => void; count: number; children: ReactNode }) {
  return (
    <button type="button" className="chip documents-chip" aria-pressed={isPressed} onClick={onPress}>
      <span className="chip-check" aria-hidden="true">
        {isPressed && <Icon name="selected" />}
      </span>
      <span className="documents-chip-label">{children}</span> <span className="chip-count">{count}</span>
    </button>
  );
}

/** A facet's head: its name and choice on a button that opens its values; off All, the one press back to All beside it. */
function FacetHead({
  name,
  summary,
  open,
  panelId,
  onToggle,
  reset,
  headRef,
}: {
  name: string;
  summary: string;
  open: boolean;
  panelId: string;
  onToggle: () => void;
  /** The All button's words and act; null at All. */
  reset: { label: string; onPress: () => void } | null;
  headRef: Ref<HTMLButtonElement>;
}) {
  const t = useTranslate();
  return (
    <div className="documents-facet">
      <button ref={headRef} type="button" className="documents-facet-button" aria-expanded={open} aria-controls={panelId} onClick={onToggle}>
        <span className="documents-facet-text">
          {fill(t("documents.facetValue", { facet: slot("facet"), value: slot("value") }), {
            facet: <span className="documents-facet-name">{name}</span>,
            value: <bdi className="documents-facet-value">{summary}</bdi>,
          })}
        </span>
        <Icon name={open ? "chevronDown" : "chevronRight"} />
      </button>
      {reset !== null && (
        <Button appearance="subtle" spacing="compact" onClick={reset.onPress}>
          {reset.label}
        </Button>
      )}
    </div>
  );
}

/** Include archive: a second read; checked while it is asked, answered or not. */
function ArchiveToggle({ archive, onArchive }: { archive: boolean; onArchive: (on: boolean) => void }) {
  const t = useTranslate();
  return (
    <label className="documents-archive">
      <input
        type="checkbox"
        checked={archive}
        onChange={(event) => {
          onArchive(event.target.checked);
        }}
      />
      <span>{t("documents.archive")}</span>
    </label>
  );
}

/** A row's Decisions cell's words, "2 accepted · 1 superseded"; a group this build does not know raw, under `lang="en"`. */
function DecisionsSummary({ decisions }: { decisions: readonly LibraryDecision[] }) {
  const t = useTranslate();
  return (
    <span>
      {decisionCounts(decisions).map((part, index) => (
        <Fragment key={part.group}>
          {index > 0 && " · "}
          {part.known === null
            ? fill(decisionCountText(part, t, slot("group")), {
                group: (
                  <span className="documents-raw" lang="en">
                    {part.group}
                  </span>
                ),
              })
            : decisionCountText(part, t, part.group)}
        </Fragment>
      ))}
    </span>
  );
}

/** The filter bar: search, the facets in their order with their open panel below, the toggles, Include archive, Clear filters. */
function FilterBar({
  rows,
  filters,
  onFilters,
  modules,
  module,
  onModule,
  archive,
  onArchive,
  onClear,
  searchRef,
}: {
  rows: readonly LibraryRow[];
  filters: LibraryFilters;
  onFilters: (next: LibraryFilters) => void;
  modules: readonly WorkspaceModule[];
  module: WorkspaceModule | null;
  onModule: (name: string) => void;
  archive: boolean;
  onArchive: (on: boolean) => void;
  onClear: () => void;
  searchRef: Ref<HTMLInputElement>;
}) {
  const t = useTranslate();
  const { language } = useLanguage();
  const collator = useMemo(() => new Intl.Collator(language, { numeric: true }), [language]);
  const base = useId();
  const focusLater = useFocusLater();
  const heads = useRef(new Map<string, HTMLButtonElement>());
  const [open, setOpen] = useState<string | null>(null);
  const chosenModule = module?.name ?? null;
  const facets = FACET_ORDER.filter((facet) => facet !== "module" || modules.length > 0);
  const panelId = (facet: string) => `${base}-${facet}`;
  const headRef = (facet: string) => (element: HTMLButtonElement | null) => {
    if (element === null) {
      heads.current.delete(facet);
    } else {
      heads.current.set(facet, element);
    }
  };
  /** Back at All: the All button goes, so focus moves to the facet's head. */
  const backTo = (facet: string) => {
    focusLater(() => heads.current.get(facet));
  };

  const head = (facet: (typeof FACET_ORDER)[number]) => {
    const name = t(`documents.facet.${facet}`);
    const toggle = () => {
      setOpen(open === facet ? null : facet);
    };
    if (facet === "module") {
      return (
        <FacetHead
          key={facet}
          name={name}
          summary={module === null ? t("documents.any") : module.title}
          open={open === facet}
          panelId={panelId(facet)}
          onToggle={toggle}
          headRef={headRef(facet)}
          reset={
            module === null
              ? null
              : {
                  label: t("documents.all.module"),
                  onPress: () => {
                    onModule(ALL_MODULES);
                    backTo(facet);
                  },
                }
          }
        />
      );
    }
    const chosen = filters.chosen[facet];
    const summary = chosen.length === 0 ? t("documents.any") : chosen.length === 1 ? valueLabel(facet, chosen[0] ?? null, t) : t("documents.chosen", { count: chosen.length });
    return (
      <FacetHead
        key={facet}
        name={name}
        summary={summary}
        open={open === facet}
        panelId={panelId(facet)}
        onToggle={toggle}
        headRef={headRef(facet)}
        reset={
          chosen.length === 0
            ? null
            : {
                label: t(`documents.all.${facet}`),
                onPress: () => {
                  onFilters(atAll(filters, facet));
                  backTo(facet);
                },
              }
        }
      />
    );
  };

  const panel = (facet: (typeof FACET_ORDER)[number]) => {
    const shown = open === facet;
    let choices: ReactNode = null;
    if (shown && facet === "module") {
      const { all, choices: listed } = moduleChoices(rows, filters, modules, collator);
      choices = (
        <>
          <Chip
            pressed={module === null}
            count={all}
            onPress={() => {
              onModule(ALL_MODULES);
            }}
          >
            {t("documents.all.module")}
          </Chip>
          {listed.map((choice) => (
            <Chip
              key={choice.module.name}
              pressed={chosenModule === choice.module.name}
              count={choice.count}
              onPress={() => {
                onModule(choice.module.name);
              }}
            >
              <bdi>{choice.module.title}</bdi>
            </Chip>
          ))}
        </>
      );
    } else if (shown && facet !== "module") {
      const chosen = filters.chosen[facet];
      choices = facetChoices(rows, filters, chosenModule, facet, collator).map((choice) => (
        <Chip
          key={choice.value === null ? "none" : `value:${choice.value}`}
          pressed={chosen.includes(choice.value)}
          count={choice.count}
          onPress={() => {
            onFilters(toggled(filters, facet, choice.value));
          }}
        >
          {choice.value === null ? valueLabel(facet, null, t) : <bdi>{choice.value}</bdi>}
        </Chip>
      ));
    }
    return (
      <div key={facet} id={panelId(facet)} className="documents-facet-panel" role="group" aria-label={t(`documents.facet.${facet}`)} hidden={!shown}>
        {choices}
      </div>
    );
  };

  const behind = useMemo(() => toggleCount(rows, filters, chosenModule, "behind"), [rows, filters, chosenModule]);
  const orphans = useMemo(() => toggleCount(rows, filters, chosenModule, "orphans"), [rows, filters, chosenModule]);
  return (
    <div className="documents-filters">
      <div className="documents-search">
        <label className="field-label" htmlFor={`${base}-search`}>
          {t("documents.search")}
        </label>
        <input
          ref={searchRef}
          id={`${base}-search`}
          className="field documents-search-field"
          type="search"
          value={filters.search}
          placeholder={t("documents.searchHint")}
          onChange={(event) => {
            onFilters({ ...filters, search: event.target.value });
          }}
        />
      </div>
      <div className="documents-facets" role="group" aria-label={t("documents.filters")}>
        {facets.map(head)}
        <Chip
          pressed={filters.behind}
          count={behind}
          onPress={() => {
            onFilters({ ...filters, behind: !filters.behind });
          }}
        >
          {t("documents.behindOnly")}
        </Chip>
        <Chip
          pressed={filters.orphans}
          count={orphans}
          onPress={() => {
            onFilters({ ...filters, orphans: !filters.orphans });
          }}
        >
          {t("documents.orphansOnly")}
        </Chip>
        <ArchiveToggle archive={archive} onArchive={onArchive} />
        {isFiltered(filters, chosenModule) && <Button onClick={onClear}>{t("documents.clear")}</Button>}
      </div>
      {facets.map(panel)}
    </div>
  );
}

/** A header: its column's name, on a subtle compact button where a press orders the rows, `aria-sort` saying how. */
function Head({ column, sort, onPress }: { column: (typeof LIBRARY_COLUMNS)[number]; sort: LibrarySort | null; onPress: (column: SortedColumn) => void }) {
  const t = useTranslate();
  const label = t(`documents.column.${column}`);
  if (!isSorted(column)) {
    return <th scope="col">{label}</th>;
  }
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
        <span>{label}</span>
        {order !== undefined && <Icon name={order === "descending" ? "cardDown" : "cardUp"} />}
      </Button>
    </th>
  );
}

/** A stored time as the viewer reads it. */
function Stamp({ at }: { at: string }) {
  const t = useTranslate();
  return (
    <time className="documents-time" dateTime={at}>
      {formatUtc(at, t)}
    </time>
  );
}

/** A row's last change: its time and who, by name alone; none read, Not committed. No OID, email or proposal is drawn. */
function LastChange({ change }: { change: LibraryChange | null }) {
  const t = useTranslate();
  if (change === null) {
    return <span className="documents-none">{t("documents.noChange")}</span>;
  }
  return <>{fill(t("documents.changed", { at: slot("at"), name: slot("name") }), { at: <Stamp at={change.at} />, name: <bdi>{change.by_name}</bdi> })}</>;
}

/** One decision under its group: its link, its `date:`, its commit time, its section, and whether it is newer than the row. */
function DecisionEntry({ project, decision }: { project: string; decision: LibraryDecision }) {
  const t = useTranslate();
  const change = decision.last_change;
  return (
    <li className="documents-decision">
      <DocLink project={project} docRef={decision.name} className="documents-decision-link">
        <bdi>{decisionTitle(decision)}</bdi>
      </DocLink>
      <span className="documents-decision-fact">{decision.date ?? t("documents.noDate")}</span>
      <span className="documents-decision-fact">{change === null ? t("documents.noChange") : fill(t("documents.committed", { at: slot("at") }), { at: <Stamp at={change.at} /> })}</span>
      {decision.section !== null && (
        <span className="documents-decision-fact">{fill(t("documents.inSection", { section: slot("section") }), { section: <span className="mono">{decision.section}</span> })}</span>
      )}
      {decision.newer === true && <Lozenge appearance="information" icon="deferred" label={t("documents.newer")} />}
    </li>
  );
}

/** The full-width row below an expanded Decisions cell: one group per served group, in served order; a group this build does not know, raw. */
function DecisionsRow({ project, row, id }: { project: string; row: LibraryRow; id: string }) {
  const t = useTranslate();
  const base = useId();
  return (
    <tr id={id} className="documents-decisions-row">
      <td colSpan={LIBRARY_COLUMNS.length}>
        <div className="documents-groups" role="group" aria-label={t("documents.decisionsOf", { title: titleOf(row) })}>
          {groupsOf(row.decisions).map(({ group, entries }, index) => {
            const { label, raw } = groupLabel(group, t);
            const labelId = `${base}-${String(index)}`;
            return (
              <div key={group} className="documents-group" role="group" aria-labelledby={labelId}>
                <p id={labelId} className="documents-group-title" lang={raw ? "en" : undefined}>
                  {label}
                </p>
                <ul className="documents-decision-list">
                  {entries.map((decision) => (
                    <DecisionEntry key={decision.path} project={project} decision={decision} />
                  ))}
                </ul>
              </div>
            );
          })}
        </div>
      </td>
    </tr>
  );
}

/** One served row; its title opens the document modal. */
function LibraryLine({ project, row, index, open, detailId, onToggle }: { project: string; row: LibraryRow; index: number; open: boolean; detailId: string; onToggle: () => void }) {
  const t = useTranslate();
  const title = titleOf(row);
  return (
    <tr data-row={row.name} data-index={index}>
      <th scope="row">
        <DocLink project={project} docRef={row.name} className="documents-link">
          <bdi>{title}</bdi>
        </DocLink>
        {title !== row.name && <span className="documents-name mono">{row.name}</span>}
      </th>
      <td>
        <Written value={row.class} />
      </td>
      <td>
        <Written value={row.kind} />
      </td>
      <td>
        <Written value={row.status} />
      </td>
      <td className="documents-figure">
        <Written value={row.rev} />
      </td>
      <td>
        <Written value={row.owner} />
      </td>
      <td className="documents-figure">
        <Written value={row.reviewed} />
      </td>
      <td className="documents-changed">
        <LastChange change={row.last_change} />
      </td>
      <td className="documents-figure documents-used">
        <UsedCell row={row} />
      </td>
      <td className="documents-decisions">
        {row.decisions.length === 0 ? (
          <span className="documents-none">-</span>
        ) : (
          <button type="button" className="documents-decisions-button" aria-expanded={open} aria-controls={open ? detailId : undefined} onClick={onToggle}>
            <Icon name={open ? "chevronDown" : "chevronRight"} />
            <DecisionsSummary decisions={row.decisions} />
          </button>
        )}
        {row.behind === true && <Lozenge appearance="warning" icon="deferred" label={t("documents.behindMark")} />}
      </td>
      <td>
        {row.tags.length === 0 ? (
          <span className="documents-none">-</span>
        ) : (
          <ul className="documents-tags">
            {row.tags.map((tag) => (
              <li key={tag} className="documents-tag">
                <bdi>{tag}</bdi>
              </li>
            ))}
          </ul>
        )}
      </td>
    </tr>
  );
}

/** The table: the first 200 of the current order, then Show 200 more, focus moving to the first row it shows. */
function LibraryTable({ project, rows, labelId, sort, onSort }: { project: string; rows: readonly LibraryRow[]; labelId: string; sort: LibrarySort | null; onSort: (next: LibrarySort) => void }) {
  const t = useTranslate();
  const { language } = useLanguage();
  const collator = useMemo(() => new Intl.Collator(language, { numeric: true }), [language]);
  const focusLater = useFocusLater();
  const base = useId();
  const body = useRef<HTMLTableSectionElement>(null);
  const [shown, setShown] = useState(LIBRARY_SHOWN);
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(() => new Set());
  const ordered = useMemo(() => sortedRows(rows, sort, collator), [rows, sort, collator]);
  const visible = ordered.slice(0, shown);
  const rest = ordered.length - visible.length;

  function more() {
    const first = visible.length;
    setShown(first + LIBRARY_SHOWN);
    focusLater(() => body.current?.querySelector<HTMLElement>(`tr[data-index="${String(first)}"] a`));
  }

  return (
    <>
      <div className="documents-table-wrap" role="region" aria-labelledby={labelId} tabIndex={0}>
        <table className="documents-table" aria-labelledby={labelId}>
          <thead>
            <tr>
              {LIBRARY_COLUMNS.map((column) => (
                <Head
                  key={column}
                  column={column}
                  sort={sort}
                  onPress={(next) => {
                    onSort(pressed(sort, next));
                  }}
                />
              ))}
            </tr>
          </thead>
          <tbody ref={body}>
            {visible.map((row, index) => {
              const open = expanded.has(row.path);
              const detailId = `${base}-${String(index)}`;
              return [
                <LibraryLine
                  key={row.path}
                  project={project}
                  row={row}
                  index={index}
                  open={open}
                  detailId={detailId}
                  onToggle={() => {
                    const next = new Set(expanded);
                    if (open) {
                      next.delete(row.path);
                    } else {
                      next.add(row.path);
                    }
                    setExpanded(next);
                  }}
                />,
                open ? <DecisionsRow key={`${row.path}#decisions`} project={project} row={row} id={detailId} /> : null,
              ];
            })}
          </tbody>
        </table>
      </div>
      {rest > 0 && <Button onClick={more}>{t("documents.more", { count: Math.min(rest, LIBRARY_SHOWN) })}</Button>}
    </>
  );
}

/** The count line: "N documents", "K of N documents" when anything narrows, then "B behind their decision" when B > 0. */
function countLine(rows: readonly LibraryRow[], kept: readonly LibraryRow[], filtered: boolean, t: Translate): string {
  const documents = filtered ? t("documents.countOf", { count: rows.length, shown: kept.length }) : t("documents.count", { count: rows.length });
  const behind = kept.filter((row) => row.behind === true).length;
  return behind === 0 ? documents : t("documents.countLine", { documents, behind: t("documents.behind", { count: behind }) });
}

/**
 * Documents: the library read while on screen, a skeleton until it and the module are known, then the
 * filter bar and the table; no document, an empty state; no match, one with Clear filters. A failure
 * in the daemon's words with Retry, an older daemon's 404 too; the last answered rows and their bar
 * stay below it, so a failed Include archive is one press from off, a line saying whether they hold the
 * archive when the checkbox says otherwise (with none answered yet, the checkbox stands alone under
 * the failure, focus following it into the bar once an answer lands). Include archive reads again with the table kept up
 * meanwhile, and is remembered for the page only once its read has answered: a failing read is never
 * asked again by coming back. The daemon's notes last.
 */
export function DocumentsView({
  project,
  memory,
  modules,
  module,
  onChooseModule,
  moduleKnown,
}: {
  project: string;
  memory: DocumentsMemory;
  /** The served modules, in served order; none hides the Module facet. */
  modules: readonly WorkspaceModule[];
  /** The stored module choice, the module bar's; null: All modules. */
  module: WorkspaceModule | null;
  /** Stores a module's name, ALL_MODULES for All modules: the module bar's choice too. */
  onChooseModule: (name: string) => void;
  /** Whether the workspaces read that gives the module has answered or failed. */
  moduleKnown: boolean;
}) {
  const t = useTranslate();
  const titleId = useId();
  const countId = useId();
  const region = useRef<HTMLElement>(null);
  const search = useRef<HTMLInputElement>(null);
  const focusLater = useFocusLater();
  const [remembered] = useState(() => memory.recall(project));
  const [archive, setArchive] = useState(remembered?.archive ?? false);
  const [held, setHeld] = useState<LibraryFilters | null>(remembered?.filters ?? null);
  const [sort, setSort] = useState<LibrarySort | null>(remembered?.sort ?? null);
  const read = useLibrary(project, archive);
  const failure = useRetainedFailure(read.error, read.isFetching);
  const retried = useRetryFocus(read.data, () => region.current?.querySelector<HTMLElement>(".documents-table-wrap") ?? region.current?.querySelector<HTMLElement>("h1"));
  // The last answer read for its own key, never a placeholder, with the archive it was asked with: a
  // failed read has no placeholder (TanStack keeps one only while pending), so this keeps the rows up.
  const [answered, setAnswered] = useState<{ document: LibraryDocument; archive: boolean } | null>(null);
  if (read.data !== undefined && !read.isPlaceholderData && answered?.document !== read.data) {
    // Storing information from previous renders (react.dev, useState).
    setAnswered({ document: read.data, archive });
  }
  const data = read.data ?? answered?.document;
  // The lone Include archive under a failure with nothing answered: unchecking it reads again, and once
  // that answer lands the checkbox moves into the bar, so focus follows it there rather than to the body.
  const toggledAlone = useRetryFocus(read.data, () => region.current?.querySelector<HTMLElement>(".documents-facets .documents-archive input"));
  /** Include archive as the page remembers it: the last answered read's; before any, the start, unless its read failed. */
  const answeredArchive = answered?.archive ?? (failure === null ? (remembered?.archive ?? false) : false);
  const rows = data?.rows;
  const filters = useMemo(() => held ?? (rows === undefined ? NO_FILTERS : startingFilters(rows)), [held, rows]);
  const chosenModule = module?.name ?? null;
  const kept = useMemo(() => (rows === undefined ? [] : keptRows(rows, filters, chosenModule)), [rows, filters, chosenModule]);
  const filtered = isFiltered(filters, chosenModule);

  useEffect(() => {
    memory.remember(project, { filters: held, archive: answeredArchive, sort });
  }, [memory, project, held, answeredArchive, sort]);

  /** Every facet at All, the toggles off, the search empty; Module at All modules, stored. Focus to the search field, the bar's first stop. */
  function clear() {
    setHeld(NO_FILTERS);
    if (chosenModule !== null) {
      onChooseModule(ALL_MODULES);
    }
    focusLater(() => search.current);
  }

  let body: ReactNode;
  if (data === undefined || rows === undefined || !moduleKnown) {
    // A failure with nothing answered: Include archive stands alone under it, mounted while the failure
    // stays up (a read again keeps it), so the checkbox a person just pressed keeps the focus.
    body =
      failure === null || data !== undefined ? (
        <Skeleton label={t("documents.loading", { project })} lines={6} />
      ) : (
        <div className="documents-filters">
          <ArchiveToggle
            archive={archive}
            onArchive={(on) => {
              toggledAlone();
              setArchive(on);
            }}
          />
        </div>
      );
  } else if (rows.length === 0) {
    body = <EmptyState level={2} title={t("documents.empty")} description={<p>{t("documents.emptyText")}</p>} />;
  } else {
    body = (
      <>
        <FilterBar
          rows={rows}
          filters={filters}
          onFilters={setHeld}
          modules={modules}
          module={module}
          onModule={onChooseModule}
          archive={archive}
          onArchive={setArchive}
          onClear={clear}
          searchRef={search}
        />
        {kept.length === 0 ? (
          <EmptyState level={2} title={t("documents.noMatch")} description={<p>{t("documents.noMatchText")}</p>} primary={<Button onClick={clear}>{t("documents.clear")}</Button>} />
        ) : (
          <LibraryTable project={project} rows={kept} labelId={countId} sort={sort} onSort={setSort} />
        )}
      </>
    );
  }

  return (
    <section ref={region} className="view documents-view" aria-labelledby={titleId} aria-busy={read.isFetching}>
      <header className="view-head">
        <h1 id={titleId} tabIndex={-1}>
          {t("documents.title")}
        </h1>
        <p className="view-about">{aboutOf("documents", t)}</p>
        {rows !== undefined && rows.length > 0 && moduleKnown && (
          <p id={countId} className="documents-count" aria-live="polite">
            {countLine(rows, kept, filtered, t)}
          </p>
        )}
      </header>
      {failure !== null && (
        <ErrorPanel
          title={t("documents.failed")}
          message={apiErrorOf(failure).message}
          retrying={read.isFetching}
          attempt={read.errorUpdateCount}
          onRetry={() => {
            retried();
            void read.refetch({ cancelRefetch: false });
          }}
        />
      )}
      {failure !== null && answered !== null && data === answered.document && answered.archive !== archive && (
        <p className="documents-shown">{answered.archive ? t("documents.lastWith") : t("documents.lastWithout")}</p>
      )}
      {body}
      {data !== undefined && data.notes.length > 0 && (
        <ul className="note-list" aria-label={t("documents.notes")}>
          {data.notes.map((note) => (
            <li key={note} lang="en">
              {note}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
