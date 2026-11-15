import { memo, useEffect, useId, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { apiErrorOf } from "../api/client";
import { FEED_DEFAULT, useFeed } from "../api/feedQueries";
import { useCachedTasks, useIdentity, useWorkspaces } from "../api/queries";
import type { FeedAuto, FeedEntry, FeedNeedsMe, TaskListEntry, WorkspaceDocument } from "../api/types";
import { pushHash } from "../app/location";
import { feedHash, sectionHash, type FeedTab } from "../app/routes";
import { useOpenShortcuts } from "../app/shortcuts";
import { useLanguage, useTranslate, type Translate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { statusLook } from "../inbox/labels";
import { byNumber } from "../tasks/groups";
import { runOutcomeLook, taskStatusLook } from "../tasks/labels";
import { useAnnounce } from "../ui/announcer";
import { Badge } from "../ui/Badge";
import { Button } from "../ui/Button";
import { EmptyState } from "../ui/EmptyState";
import { Icon } from "../ui/Icon";
import { hasModifier, isTextField } from "../ui/keys";
import { Lozenge } from "../ui/Lozenge";
import { ErrorPanel, ReadFailure, Skeleton } from "../ui/states";
import { TabPanel, Tabs } from "../ui/Tabs";
import { shownSlot, shownText } from "../ui/text";
import { formatUtc } from "../ui/time";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { recallSeen, rememberSeen } from "./seen";
import { actLabel, byLabel, proposalKindLabel, quietBetween, reasonLabel, shortCommit, shortRule, subjectOf, typeLabel, type CodeLabel } from "./words";

// Activity, the feed (docs/features/auto-feed.md "The UI"): `#/<p>/feed` and `#/<p>/feed/needs-me`,
// the nav's second entry (the owner's word of 2026-11-15). One read of `spec feed --needs-me` with the view's filters, appended by every live
// event; All lists its entries newest first, Needs me its five lists. It decides nothing: each
// subject links to where it is acted on. What the viewer last saw is kept per viewer, read when the
// view opens and written when it closes or on Mark all seen; without storage the marks hold here.

/** One formatter per language, made once: a day and a time in the viewer's language and time zone. */
const LOCAL_TIMES = new Map<string, Intl.DateTimeFormat>();

/** A stored time in the viewer's language and time zone, e.g. `Nov 16, 01:02` in English; raw when it does not parse. */
function localTime(stored: string, language: string): string {
  const at = Date.parse(stored);
  if (Number.isNaN(at)) {
    return stored;
  }
  let format = LOCAL_TIMES.get(language);
  if (format === undefined) {
    format = new Intl.DateTimeFormat(language, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit", hourCycle: "h23" });
    LOCAL_TIMES.set(language, format);
  }
  return format.format(at);
}

/** A stored time: the viewer's local time shown, the UTC stamp its title; none, nothing. */
function When({ at }: { at: string | null }) {
  const t = useTranslate();
  const { language } = useLanguage();
  if (at === null) {
    return null;
  }
  return (
    <time className="feed-time" dateTime={at} title={formatUtc(at, t)}>
      {localTime(at, language)}
    </time>
  );
}

/** An ID that names where it is acted on: an agent task's Tasks detail, a proposal's view, a task's modal on the Board. */
function SubjectLink({ project, to, id }: { project: string; to: "tasks" | "inbox" | "board"; id: string }) {
  return (
    <a className="mono feed-subject" href={sectionHash(project, to, id)}>
      {shownSlot(id)}
    </a>
  );
}

/** A served code in the viewer's words, the code itself its title; a code no table knows raw, as code; none, nothing. */
function Coded({ code, said }: { code: string | null; said: CodeLabel | null }) {
  if (code === null || said === null) {
    return null;
  }
  return said.raw ? (
    <code lang="en">{shownSlot(code)}</code>
  ) : (
    <span className="feed-code" title={code}>
      {said.label}
    </span>
  );
}

/** Newest first by a stored time, ties by ID number, the highest first; an item with no time last. */
function newestFirst<T>(rows: readonly T[], at: (row: T) => string | null, id: (row: T) => string): T[] {
  return [...rows].sort((a, b) => {
    const [x, y] = [at(a), at(b)];
    const later = x === null || y === null ? (x === null ? 1 : 0) - (y === null ? 1 : 0) : Date.parse(y) - Date.parse(x);
    return Number.isNaN(later) || later === 0 ? byNumber(id(b), id(a)) : later;
  });
}

/** A commit as its twelve hex digits, the whole commit its title. */
function Commit({ commit }: { commit: string }) {
  return (
    <code className="feed-commit" title={commit}>
      {shortCommit(commit)}
    </code>
  );
}

/** The head line from the auto head: on with its rule, or off with the daemon's why. */
function AutoHead({ auto }: { auto: FeedAuto }) {
  const t = useTranslate();
  if (auto.state === "on") {
    const rule = auto.rule ?? "";
    return (
      <p className="feed-auto">
        <Icon name="terminal" />
        <span>
          {fill(t("feed.autoOn", { hash: slot("hash") }), {
            hash: (
              <code title={rule}>
                <span aria-hidden="true">{shownSlot(shortRule(rule))}</span>
                <span className="sr-only">{shownSlot(rule)}</span>
              </code>
            ),
          })}
        </span>
      </p>
    );
  }
  return (
    <p className="feed-auto">
      <Icon name="info" />
      <span>{fill(t("feed.autoOff", { why: slot("why") }), { why: <span lang="en">{shownSlot(auto.why ?? auto.reason ?? auto.state)}</span> })}</span>
    </p>
  );
}

/** The state now of an entry's agent task, else its proposal: the screens' own labels, a word no table knows raw. */
function statusOf(entry: FeedEntry, t: Translate): ReactNode {
  if (entry.status === null) {
    return null;
  }
  return <Badge name={t("tasks.stateName")} look={entry.task !== null ? taskStatusLook(entry.status, t) : statusLook(entry.status, t)} />;
}

/** One entry: its time, its type, its subject, its marks, its title and whatever its keys hold. */
// Memoized: a live event appends entries, and the held ones, the same objects, are not drawn again.
const EntryRow = memo(function EntryRow({ project, entry, fresh }: { project: string; entry: FeedEntry; fresh: boolean }) {
  const t = useTranslate();
  const { label, raw } = typeLabel(entry.type, t);
  const subject = subjectOf(entry);
  const by = byLabel(entry.by, t);
  // A held proposal or a bound line beside its agent task: named too, as the CLI's line keeps it.
  const proposalBeside = entry.task !== null && entry.proposal !== null;
  const lineBeside = subject?.to !== "line" && entry.line !== null;
  // A daemon's words: an apply refused and a hold say why in English; a rejection is the person's own.
  const daemonWords = entry.type !== "proposal.rejected";
  return (
    <li className="feed-entry" data-seq={entry.seq} data-new={fresh ? "true" : undefined}>
      <div className="feed-entry-head">
        <When at={entry.at} />
        <span className={raw ? "feed-type mono" : "feed-type"} lang={raw ? "en" : undefined}>
          {label}
        </span>
        {subject !== null && (subject.to === "line" ? <span className="mono feed-subject">{shownSlot(subject.id)}</span> : <SubjectLink project={project} to={subject.to} id={subject.id} />)}
        {proposalBeside && entry.proposal !== null && <SubjectLink project={project} to="inbox" id={entry.proposal} />}
        {lineBeside && entry.line !== null && <span className="mono feed-subject">{shownSlot(entry.line)}</span>}
        {fresh && <Lozenge appearance="discovery" icon="info" label={t("feed.new")} />}
        {by !== null && <Lozenge appearance="information" icon="terminal" label={by.label} lang={by.raw ? "en" : undefined} />}
        {statusOf(entry, t)}
      </div>
      {entry.title !== null && (
        <p className="feed-title">
          <bdi>{shownSlot(entry.title)}</bdi>
        </p>
      )}
      {(entry.outcome !== null || entry.summary !== null || entry.commit !== null || entry.reason !== null || entry.detail !== null || entry.path !== null) && (
        <div className="feed-words">
          {entry.outcome !== null && <Badge look={runOutcomeLook(entry.outcome, t)} />}
          <Coded code={entry.reason} said={reasonLabel(entry.reason, t)} />
          {entry.commit !== null && <Commit commit={entry.commit} />}
          {entry.path !== null && <code>{shownSlot(entry.path)}</code>}
          {entry.summary !== null && <p className="feed-text">{shownText(entry.summary)}</p>}
          {entry.detail !== null && (
            <p className="feed-text" lang={daemonWords ? "en" : undefined}>
              {shownText(entry.detail)}
            </p>
          )}
        </div>
      )}
    </li>
  );
});

/** A stretch of two hours or more with no entry between two shown: a pause, a usage limit's among them. */
function Quiet({ earlier, later }: { earlier: FeedEntry; later: FeedEntry }) {
  const t = useTranslate();
  const quiet = quietBetween(earlier.at, later.at);
  if (quiet === null) {
    return null;
  }
  return (
    // Not an entry: no list item to count, its words still read.
    <li className="feed-quiet" role="presentation">
      <Icon name="waiting" />
      <span>{t("feed.quiet", { hours: quiet.hours, minutes: quiet.minutes })}</span>
    </li>
  );
}

/** One list of Needs me: its heading, its rows, or that it holds none. */
function NeedsList({ title, rows }: { title: string; rows: ReactNode[] }) {
  const t = useTranslate();
  const headingId = useId();
  return (
    <section className="feed-needs" aria-labelledby={headingId}>
      <h2 id={headingId} className="feed-needs-title">
        <span>{title}</span> <span className="chip-count">{rows.length}</span>
      </h2>
      {rows.length === 0 ? <p className="muted">{t("ui.emptyValue")}</p> : <ul className="feed-needs-list">{rows}</ul>}
    </section>
  );
}

/** The five lists of what needs the owner, each empty one saying so. */
function NeedsMe({ project, needs }: { project: string; needs: FeedNeedsMe | null }) {
  const t = useTranslate();
  // Every list newest first, as All is: by its time, ties by ID number, the highest first.
  const held = newestFirst(needs?.held ?? [], (row) => row.at, (row) => String(row.seq));
  const open = newestFirst(needs?.open ?? [], (row) => row.created_at, (row) => row.id);
  const failed = newestFirst(needs?.failed_runs ?? [], (row) => row.ended_at, (row) => `${row.task}.${String(row.run)}`);
  const gate = newestFirst(needs?.gate_observed ?? [], (row) => row.at, (row) => String(row.seq));
  const unlanded = newestFirst(needs?.unlanded ?? [], () => null, (row) => row.task);
  return (
    <div className="feed-needs-all">
      <NeedsList
        title={t("feed.type.held")}
        rows={held.map((row) => (
          <li key={row.seq} className="feed-row">
            <When at={row.at} />
            {row.task !== null && <SubjectLink project={project} to="tasks" id={row.task} />}
            {row.proposal !== null && <SubjectLink project={project} to="inbox" id={row.proposal} />}
            {row.task === null && row.proposal === null && row.line !== null && <span className="mono feed-subject">{shownSlot(row.line)}</span>}
            <Coded code={row.act} said={actLabel(row.act, t)} />
            <Coded code={row.reason} said={reasonLabel(row.reason, t)} />
            {row.detail !== null && (
              <p className="feed-text" lang="en">
                {shownText(row.detail)}
              </p>
            )}
          </li>
        ))}
      />
      <NeedsList
        title={t("feed.needs.open")}
        rows={open.map((row) => (
          <li key={row.id} className="feed-row">
            <When at={row.created_at} />
            <SubjectLink project={project} to="inbox" id={row.id} />
            <Coded code={row.kind} said={proposalKindLabel(row, t)} />
            {row.target_id !== null && <code>{shownSlot(row.target_id)}</code>}
            {row.task_id !== null && <SubjectLink project={project} to="tasks" id={row.task_id} />}
          </li>
        ))}
      />
      <NeedsList
        title={t("feed.needs.failed")}
        rows={failed.map((row) => (
          <li key={`${row.task}/${String(row.run)}`} className="feed-row">
            {row.ended_at !== null && <When at={row.ended_at} />}
            <SubjectLink project={project} to="tasks" id={row.task} />
            <span>{t("tasks.runs.run", { run: row.run })}</span>
            {row.outcome !== null && <Badge look={runOutcomeLook(row.outcome, t)} />}
            {row.summary !== null && <p className="feed-text">{shownText(row.summary)}</p>}
          </li>
        ))}
      />
      <NeedsList
        title={t("feed.needs.gate")}
        rows={gate.map((row) => (
          <li key={row.seq} className="feed-row">
            <When at={row.at} />
            {row.path !== null && <code>{shownSlot(row.path)}</code>}
            {row.zone !== null && <code>{shownSlot(row.zone)}</code>}
            {row.branch !== null && <code>{shownSlot(row.branch)}</code>}
          </li>
        ))}
      />
      <NeedsList
        title={t("feed.needs.unlanded")}
        rows={unlanded.map((row) => (
          <li key={row.task} className="feed-row">
            <SubjectLink project={project} to="tasks" id={row.task} />
            <Badge name={t("tasks.stateName")} look={taskStatusLook(row.status, t)} />
          </li>
        ))}
      />
    </div>
  );
}

/** How many items Needs me lists. */
function needsCount(needs: FeedNeedsMe | null): number {
  return needs === null ? 0 : needs.held.length + needs.open.length + needs.failed_runs.length + needs.gate_observed.length + needs.unlanded.length;
}

/** The departments a project declares, its served `[[workspace]]` names; none for the implicit one. */
function departmentsOf(document: WorkspaceDocument | undefined): { name: string; title: string }[] {
  return document?.source === "config" ? document.workspaces.map(({ name, title }) => ({ name, title })) : [];
}

/** The highest `seq` among entries; null with none. */
function newestOf(entries: readonly FeedEntry[]): number | null {
  return entries.reduce<number | null>((top, entry) => (top === null || entry.seq > top ? entry.seq : top), null);
}

/**
 * The Activity view. `tab` is the route's: All, or Needs me. Its filters are this view's state: New
 * since your last visit (`since` the stored point), an agent task, a department where the project
 * declares departments. Nothing here writes to the daemon.
 */
export function FeedView({ project, tab }: { project: string; tab: FeedTab }) {
  const t = useTranslate();
  const openShortcuts = useOpenShortcuts();
  const announce = useAnnounce();
  const titleId = useId();
  const tabsId = useId();
  const taskId = useId();
  const departmentId = useId();
  const identity = useIdentity();
  const identityFailure = useRetainedFailure(identity.error, identity.isFetching);
  /** Whose point it is: a hub's login, null locally; undefined while the identity is out or failed, and then nothing is stored. */
  const whose = identity.data?.login;
  /** The point read when the view opened: null with none kept, so every entry is new; the view's own state once marked. */
  const [opened, setOpened] = useState<{ seq: number | null } | null>(null);
  if (opened === null && (whose !== undefined || identityFailure !== null)) {
    setOpened({ seq: whose === undefined ? null : recallSeen(project, whose) });
  }
  const [onlyNew, setOnlyNew] = useState(false);
  const [task, setTask] = useState<string | null>(null);
  const [department, setDepartment] = useState<string | null>(null);
  const since = onlyNew && opened !== null ? opened.seq : null;
  const feed = useFeed(project, { task, department, since });
  const failure = useRetainedFailure(feed.error, feed.isFetching);
  const tasks = useCachedTasks(project);
  const workspaces = useWorkspaces(project);
  const entries = feed.data?.entries ?? [];

  // The highest `seq` this view has shown, kept for its close.
  const shown = useRef<number | null>(null);
  const top = newestOf(entries);
  useEffect(() => {
    if (top !== null && (shown.current === null || top > shown.current)) {
      shown.current = top;
    }
  }, [top]);
  const login = useRef(whose);
  useEffect(() => {
    login.current = whose;
  }, [whose]);

  // Closing the view (another route, another project, the page going away) keeps the highest shown,
  // never lower than what is kept; nothing is written on open.
  useEffect(() => {
    const keep = () => {
      const viewer = login.current;
      const highest = shown.current;
      if (viewer === undefined || highest === null) {
        return;
      }
      const kept = recallSeen(project, viewer);
      rememberSeen(project, viewer, kept === null ? highest : Math.max(kept, highest));
    };
    window.addEventListener("pagehide", keep);
    return () => {
      window.removeEventListener("pagehide", keep);
      keep();
    };
  }, [project]);

  function isNew(entry: FeedEntry): boolean {
    return opened !== null && (opened.seq === null || entry.seq > opened.seq);
  }

  function markAllSeen() {
    const kept = whose === undefined ? null : recallSeen(project, whose);
    // Never lower than what is kept, as the close keeps it.
    const highest = Math.max(shown.current ?? -1, top ?? -1, opened?.seq ?? -1, kept ?? -1);
    if (highest < 0) {
      return;
    }
    setOpened({ seq: highest });
    if (whose !== undefined) {
      rememberSeen(project, whose, highest);
    }
    announce(t("feed.seenSaid"));
  }

  function onKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.defaultPrevented || hasModifier(event) || isTextField(event.target)) {
      return;
    }
    if (event.key === "?") {
      event.preventDefault();
      openShortcuts();
    }
  }

  const departments = departmentsOf(workspaces.data);
  const taskOptions: TaskListEntry[] = [...(tasks.data?.tasks ?? [])].sort((a, b) => byNumber(a.id, b.id));
  const newCount = entries.filter(isNew).length;
  // The newest 100 alone were read and every one is new: more may be, so the count says so.
  const oldest = entries[0];
  const newShown = feed.data?.after === null && newCount >= FEED_DEFAULT && oldest !== undefined && isNew(oldest) ? `${String(newCount)}+` : String(newCount);
  const needs = feed.data?.needs_me ?? null;
  const shownRows = [...entries].reverse();

  let all: ReactNode = null;
  if (feed.data !== undefined && shownRows.length === 0) {
    all = <EmptyState level={2} title={since !== null ? t("feed.noneNew") : t("feed.none")} />;
  } else if (feed.data !== undefined) {
    all = (
      <ol className="feed-list">
        {shownRows.map((entry, index) => {
          const earlier = shownRows[index + 1];
          return [
            <EntryRow key={entry.seq} project={project} entry={entry} fresh={isNew(entry)} />,
            earlier === undefined ? null : <Quiet key={`quiet-${String(entry.seq)}`} earlier={earlier} later={entry} />,
          ];
        })}
      </ol>
    );
  }

  return (
    <section className="view feed-view" aria-labelledby={titleId} onKeyDown={onKeyDown}>
      <header className="view-head">
        <h1 id={titleId} tabIndex={-1}>
          {t("feed.title")}
        </h1>
        <p className="view-about">{t("feed.about")}</p>
        {feed.data !== undefined && <AutoHead auto={feed.data.auto} />}
      </header>
      <div className="feed-controls" role="group" aria-label={t("feed.title")}>
        <button
          type="button"
          className="chip"
          aria-pressed={onlyNew}
          onClick={() => {
            setOnlyNew((current) => !current);
          }}
        >
          <span className="chip-check" aria-hidden="true">
            {onlyNew && <Icon name="selected" />}
          </span>
          <span>{t("feed.newOnly")}</span> <span className="chip-count">{newShown}</span>
        </button>
        <div className="feed-tool">
          <label className="field-label" htmlFor={taskId}>
            {t("inbox.card.task")}
          </label>
          <select
            id={taskId}
            className="field"
            value={task ?? ""}
            onChange={(event) => {
              setTask(event.target.value === "" ? null : event.target.value);
            }}
          >
            <option value="">{t("feed.anyTask")}</option>
            {task !== null && !taskOptions.some((entry) => entry.id === task) && <option value={task}>{task}</option>}
            {taskOptions.map((entry) => (
              <option key={entry.id} value={entry.id}>
                {entry.title === null ? entry.id : `${entry.id} ${shownSlot(entry.title)}`}
              </option>
            ))}
          </select>
        </div>
        {departments.length > 0 && (
          <div className="feed-tool">
            <label className="field-label" htmlFor={departmentId}>
              {t("feed.department")}
            </label>
            <select
              id={departmentId}
              className="field"
              value={department ?? ""}
              onChange={(event) => {
                setDepartment(event.target.value === "" ? null : event.target.value);
              }}
            >
              <option value="">{t("feed.anyDepartment")}</option>
              {departments.map((entry) => (
                <option key={entry.name} value={entry.name}>
                  {entry.title}
                </option>
              ))}
              {!departments.some((entry) => entry.name === "general") && <option value="general">{t("feed.general")}</option>}
            </select>
          </div>
        )}
        <Button icon="selected" onClick={markAllSeen}>
          {t("feed.seen")}
        </Button>
      </div>
      {feed.data !== undefined && feed.data.notes.length > 0 && (
        <ul className="note-list" aria-label={t("inbox.notes")}>
          {feed.data.notes.map((note, index) => (
            <li key={`${String(index)}-${note}`} className="verbatim-note" lang="en">
              {note}
            </li>
          ))}
        </ul>
      )}
      {feed.data?.more === true && (
        <p>
          <Button
            icon="arrowRight"
            onClick={() => {
              void feed.refetch({ cancelRefetch: false });
            }}
          >
            {t("feed.more")}
          </Button>
        </p>
      )}
      {feed.data === undefined &&
        (failure === null ? (
          <Skeleton label={t("feed.loading", { project })} lines={6} />
        ) : (
          <ReadFailure
            title={t("feed.failed")}
            failure={failure}
            retrying={feed.isFetching}
            attempt={feed.errorUpdateCount}
            onRetry={() => {
              void feed.refetch({ cancelRefetch: false });
            }}
          />
        ))}
      {feed.data !== undefined && failure !== null && (
        <ErrorPanel
          title={t("feed.failed")}
          message={apiErrorOf(failure).message}
          retrying={feed.isFetching}
          attempt={feed.errorUpdateCount}
          onRetry={() => {
            void feed.refetch({ cancelRefetch: false });
          }}
        />
      )}
      <Tabs
        label={t("feed.title")}
        base={tabsId}
        tabs={[
          { id: "all", label: t("feed.all") },
          {
            id: "needs-me",
            label: (
              <>
                {t("feed.needsMe")} <span className="chip-count">{needsCount(needs)}</span>
              </>
            ),
          },
        ]}
        selected={tab}
        onSelect={(next) => {
          pushHash(feedHash(project, next));
        }}
      />
      <TabPanel base={tabsId} id="all" selected={tab === "all"}>
        {all}
      </TabPanel>
      <TabPanel base={tabsId} id="needs-me" selected={tab === "needs-me"}>
        {/* Drawn while its tab shows alone; the tab's count stands either way. */}
        {tab === "needs-me" && feed.data !== undefined && <NeedsMe project={project} needs={needs} />}
      </TabPanel>
    </section>
  );
}
