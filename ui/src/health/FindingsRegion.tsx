import { memo, useId, useMemo, useRef, useState, type KeyboardEvent, type MouseEvent } from "react";
import type { CheckQuery } from "../api/queries";
import type { CheckReport } from "../api/types";
import { openDoc } from "../app/location";
import { Badge } from "../ui/Badge";
import { Button } from "../ui/Button";
import { EmptyState } from "../ui/EmptyState";
import { Icon } from "../ui/Icon";
import { hasModifier, hotkeyOf, isTextField } from "../ui/keys";
import { ReadFailure, Skeleton } from "../ui/states";
import { useFocusLater } from "../ui/useFocusLater";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import {
  codeChipsOf,
  groupsOf,
  isFiltered,
  isSpecPath,
  NO_FILTERS,
  rowsOf,
  searchableRows,
  severityChipsOf,
  SHOWN_OPEN,
  toggled,
  visibleRows,
  type Chip,
  type FindingFilters,
  type FindingGroup,
  type FindingRow,
} from "./findings";
import { findingSeverityLook, isCannotCheck } from "./labels";
import { Counted, DebtNote, FindingMessage, FindingPlace, FindingSubject, HealthRegion } from "./parts";
import { useTranslate } from "../i18n";

function Chips({ label, chips, filters, onToggle }: { label: string; chips: Chip[]; filters: FindingFilters; onToggle: (key: string) => void }) {
  return (
    <div className="finding-chips" role="group" aria-label={label}>
      {chips.map((chip) => {
        const pressed = !filters.off.has(chip.key);
        return (
          <button
            key={chip.key}
            type="button"
            className="chip"
            aria-pressed={pressed}
            onClick={() => {
              onToggle(chip.key);
            }}
          >
            <span className="chip-check" aria-hidden="true">
              {pressed && <Icon name="selected" />}
            </span>
            <span>{chip.label}</span> <span className="chip-count">{chip.count}</span>
          </button>
        );
      })}
    </div>
  );
}

/**
 * One finding: severity, where, subject, the message verbatim, its debt, the fix as text (never
 * applied). Memoised: a move re-renders the two rows whose Tab stop changes, not every open row.
 */
const FindingItem = memo(function FindingItem({
  project,
  id,
  row,
  tabbable,
  onFocus,
}: {
  project: string;
  /** The row's own id: it names itself by its text, so focus on it reads the finding. */
  id: string;
  row: FindingRow;
  tabbable: boolean;
  onFocus: (key: number) => void;
}) {
  const t = useTranslate();
  const { finding } = row;
  return (
    <li
      id={id}
      aria-labelledby={id}
      className="finding-row"
      tabIndex={tabbable ? 0 : -1}
      data-finding={row.key}
      onFocus={() => {
        onFocus(row.key);
      }}
    >
      <span className="finding-head">
        <Badge name={t("health.severityName")} look={findingSeverityLook(finding.severity, t)} />
        <FindingPlace project={project} finding={finding} linked />
        <FindingSubject subject={finding.subject} />
      </span>
      <FindingMessage message={finding.message} />
      {finding.debt !== undefined && <DebtNote debt={finding.debt} />}
      {finding.fix !== undefined && (
        <p className="finding-fix">
          <span className="finding-fix-label">{t("health.findings.fixLabel")}</span>{" "}
          <code className="finding-fix-text verbatim">{finding.fix.text}</code>
        </p>
      )}
    </li>
  );
});

/**
 * The findings of one report: chips per severity and code and a text field over that report (no
 * read), the groups by code, errors first, a group over ten rows collapsed until opened. The rows
 * are one Tab stop: Up and Down (k, j), Home and End move it; Enter on a row whose file is a spec
 * document opens that node in the tree (one history entry).
 */
function FindingList({ project, report }: { project: string; report: CheckReport }) {
  const t = useTranslate();
  const baseId = useId();
  const focusLater = useFocusLater();
  const groupsArea = useRef<HTMLDivElement>(null);
  const field = useRef<HTMLInputElement>(null);
  const [filters, setFilters] = useState<FindingFilters>(NO_FILTERS);
  /** Groups the owner opened or closed, by code; any other is open up to SHOWN_OPEN rows. */
  const [opened, setOpened] = useState<ReadonlyMap<string, boolean>>(new Map());
  const [active, setActive] = useState<number | null>(null);

  const rows = useMemo(() => searchableRows(rowsOf(report.findings)), [report.findings]);
  const severityChips = useMemo(() => severityChipsOf(rows, t), [rows, t]);
  const codeChips = useMemo(() => codeChipsOf(rows), [rows]);
  const groups = useMemo(() => groupsOf(visibleRows(rows, filters)), [rows, filters]);

  if (rows.length === 0) {
    return (
      // One level below the region's heading (docs/features/design-sweep-b.md "Acceptance criteria", DB1).
      <EmptyState level={3} title={isCannotCheck(report.verdict) ? t("health.findings.noneCannot") : t("health.findings.none")} />
    );
  }

  const isOpen = (group: FindingGroup) => opened.get(group.code) ?? group.rows.length <= SHOWN_OPEN;
  const order = groups.flatMap((group) => (isOpen(group) ? group.rows : []));
  const roving = order.find((row) => row.key === active) ?? order[0];
  const shown = groups.reduce((total, group) => total + group.rows.length, 0);

  function rowElement(key: number): HTMLElement | null {
    return groupsArea.current?.querySelector<HTMLElement>(`[data-finding="${String(key)}"]`) ?? null;
  }

  function moveTo(row: FindingRow | undefined) {
    if (row === undefined) {
      return;
    }
    setActive(row.key);
    focusLater(() => rowElement(row.key));
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (hasModifier(event) || isTextField(event.target) || !(event.target instanceof HTMLElement)) {
      return;
    }
    // A link inside a row follows itself; the keys below are the rows'.
    if (event.target instanceof HTMLAnchorElement) {
      return;
    }
    const key = event.target.closest<HTMLElement>("[data-finding]")?.dataset.finding;
    const at = order.findIndex((row) => String(row.key) === key);
    const here = order[at];
    if (here === undefined) {
      return;
    }
    switch (hotkeyOf(event)) {
      case "ArrowDown":
      case "j":
        moveTo(order[at + 1]);
        break;
      case "ArrowUp":
      case "k":
        moveTo(order[at - 1]);
        break;
      case "Home":
        moveTo(order[0]);
        break;
      case "End":
        moveTo(order[order.length - 1]);
        break;
      case "Enter":
        if (!isSpecPath(here.finding.path)) {
          return;
        }
        openDoc(here.finding.path);
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  function toggleGroup(group: FindingGroup, event: MouseEvent<HTMLButtonElement>) {
    const open = isOpen(group);
    const toggle = event.currentTarget;
    // A click leaves focus where it was in Safari (buttons take none): a row of this group may hold
    // it. Its rows go, so the toggle takes the focus rather than the page's body.
    const rowsOfGroup = toggle.id === "" ? null : document.getElementById(`${toggle.id}-rows`);
    if (open && rowsOfGroup?.contains(document.activeElement) === true) {
      focusLater(() => toggle);
    }
    setOpened((current) => new Map(current).set(group.code, !open));
  }

  function clearFilters() {
    setFilters(NO_FILTERS);
    // The button goes with the message: the rows' Tab stop or the field takes the focus.
    focusLater(() => groupsArea.current?.querySelector<HTMLElement>('[data-finding][tabindex="0"]') ?? field.current);
  }

  return (
    <>
      <div className="finding-filters" role="group" aria-label={t("health.findings.filterLabel")}>
        <Chips
          label={t("health.findings.bySeverity")}
          chips={severityChips}
          filters={filters}
          onToggle={(key) => {
            setFilters((current) => toggled(current, key));
          }}
        />
        <Chips
          label={t("health.findings.byCode")}
          chips={codeChips}
          filters={filters}
          onToggle={(key) => {
            setFilters((current) => toggled(current, key));
          }}
        />
        <label className="field-label" htmlFor={`${baseId}-filter`}>
          {t("health.findings.filterField")}
        </label>
        <input
          id={`${baseId}-filter`}
          ref={field}
          className="field"
          type="search"
          autoComplete="off"
          spellCheck={false}
          value={filters.query}
          onChange={(event) => {
            const query = event.target.value;
            setFilters((current) => ({ ...current, query }));
          }}
        />
      </div>
      <p className="finding-total">
        {isFiltered(filters) ? t("health.findings.totalShown", { shown, count: rows.length }) : t("health.findings.total", { count: rows.length })}
      </p>
      {groups.length === 0 ? (
        <EmptyState
          level={3}
          title={t("health.findings.noMatch")}
          primary={
            <Button appearance="primary" onClick={clearFilters}>
              {t("health.findings.clearFilters")}
            </Button>
          }
        />
      ) : (
        <div ref={groupsArea} className="finding-groups" onKeyDown={onKeyDown}>
          {groups.map((group, index) => {
            const open = isOpen(group);
            const headId = `${baseId}-g${String(index)}`;
            return (
              <div key={group.code} role="group" className="finding-group" aria-labelledby={headId} data-code={group.code}>
                <h3 className="finding-group-head">
                  <button
                    id={headId}
                    type="button"
                    className="finding-group-toggle"
                    aria-expanded={open}
                    aria-controls={open ? `${headId}-rows` : undefined}
                    onClick={(event) => {
                      toggleGroup(group, event);
                    }}
                  >
                    <Icon name={open ? "chevronDown" : "chevronRight"} />
                    <code className="finding-code">{group.code}</code>
                    <Counted count={group.rows.length} said={t("health.findings.counted", { count: group.rows.length })} />
                  </button>
                </h3>
                {open && (
                  <ul id={`${headId}-rows`} className="finding-rows" aria-labelledby={headId}>
                    {group.rows.map((row) => (
                      <FindingItem
                        key={row.key}
                        project={project}
                        id={`${baseId}-f${String(row.key)}`}
                        row={row}
                        tabbable={row === roving}
                        onFocus={setActive}
                      />
                    ))}
                  </ul>
                )}
              </div>
            );
          })}
        </div>
      )}
    </>
  );
}

/**
 * Region 3, Findings (docs/features/ui-health.md "Description and interactions"): the check's
 * findings as sent, grouped and filtered over the one report read; its states are the check's.
 */
export function FindingsRegion({ project, check }: { project: string; check: CheckQuery }) {
  const t = useTranslate();
  const failure = useRetainedFailure(check.error, check.isFetching);
  const heading = useRef<HTMLHeadingElement>(null);
  const retried = useRetryFocus(check.data, () => heading.current);

  let body;
  if (check.data === undefined) {
    body =
      failure === null ? (
        <Skeleton label={t("health.findings.loading", { project })} lines={5} />
      ) : (
        <ReadFailure
          title={t("health.findings.failed")}
          failure={failure}
          retrying={check.isFetching}
          attempt={check.errorUpdateCount}
          announce={false}
          onRetry={() => {
            retried();
            void check.refetch({ cancelRefetch: false });
          }}
        />
      );
  } else {
    body = <FindingList project={project} report={check.data} />;
  }

  return (
    <HealthRegion
      title={t("health.findings.title")}
      boundary={t("health.findings.boundary")}
      headingRef={heading}
      busy={check.data === undefined ? failure === null : check.isFetching}
      className="health-findings"
    >
      {body}
    </HealthRegion>
  );
}
