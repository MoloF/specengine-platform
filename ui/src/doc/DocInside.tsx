import { useEffect, useId, useMemo, useRef, useState } from "react";
import { apiErrorOf } from "../api/client";
import { useHeldWorkspaces, useInside } from "../api/queries";
import { sectionHash } from "../app/routes";
import { useTranslate } from "../i18n";
import { Button } from "../ui/Button";
import { Disclosure } from "../ui/Disclosure";
import { ErrorPanel, Skeleton } from "../ui/states";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { DocLink } from "./DocLink";
import type { PartLevel } from "./DocLinks";
import { INSIDE_SHOWN, insideDirections, insideOf, type InsideGroup, type InsideSet } from "./inside";

// What a document consists of (docs/features/doc-modal.md "Acceptance criteria", DM-13): collapsed,
// read on its first expansion only, `getTree({root: REF, depth: 2})`, never on open; a second
// expansion reads nothing. The choice to keep it open is the Shell's, per project for the page's life.
// Directions group by the workspaces read the Shell holds, never read here: while it is out or
// failed, the groups are raw kinds at once (docs/features/departments.md "Acceptance criteria", DP-13).

/** A set's label: a direction's title, the project's words with no `lang`; a raw kind under `lang="en"`; no kind, the chrome's words. */
function SetLabel({ id, set }: { id: string; set: InsideSet }) {
  const t = useTranslate();
  if (set.by === "direction") {
    return (
      <p id={id} className="doc-subheading">
        <bdi>{set.title}</bdi>
      </p>
    );
  }
  return set.kind === null ? (
    <p id={id} className="doc-subheading">
      {t("doc.inside.noKind")}
    </p>
  ) : (
    <p id={id} className="doc-subheading" lang="en">
      <bdi>{set.kind}</bdi>
    </p>
  );
}

/** One set's entries: the first INSIDE_SHOWN, then Show all, which takes focus to the first it shows. */
function SetEntries({ project, set }: { project: string; set: InsideSet }) {
  const t = useTranslate();
  const id = useId();
  const [all, setAll] = useState(false);
  const list = useRef<HTMLUListElement>(null);
  useEffect(() => {
    if (all) {
      Array.from(list.current?.querySelectorAll<HTMLElement>("a") ?? [])[INSIDE_SHOWN]?.focus();
    }
  }, [all]);
  const { entries } = set;
  const shown = all ? entries : entries.slice(0, INSIDE_SHOWN);
  return (
    <div className="doc-inside-kind" role="group" aria-labelledby={id}>
      <SetLabel id={id} set={set} />
      <ul ref={list} className="doc-inside-list">
        {shown.map((entry) => (
          <li key={`${entry.path}:${entry.name}`}>
            <DocLink project={project} docRef={entry.name}>
              <bdi>{entry.title ?? entry.name}</bdi>
            </DocLink>
          </li>
        ))}
      </ul>
      {!all && entries.length > INSIDE_SHOWN && (
        <Button
          appearance="subtle"
          spacing="compact"
          onClick={() => {
            setAll(true);
          }}
        >
          {t("doc.inside.showAll", { count: entries.length })}
        </Button>
      )}
    </div>
  );
}

/** One holder's children: a section, its heading one below Inside's, then its directions and kinds. */
function GroupPart({ project, group, level }: { project: string; group: InsideGroup; level: PartLevel }) {
  const t = useTranslate();
  const id = useId();
  const Heading = level === 2 ? "h3" : "h4";
  return (
    <section className="doc-inside-group" aria-labelledby={id}>
      <Heading id={id} className="doc-inside-holder">
        {group.direct ? t("doc.inside.direct") : t("doc.inside.under", { title: group.title ?? group.holder })}
      </Heading>
      {group.sets.map((set) => (
        <SetEntries key={set.by === "direction" ? `direction:${set.name}` : `kind:${set.kind ?? ""}`} project={project} set={set} />
      ))}
    </section>
  );
}

/** The read once asked: a skeleton, the daemon's words and Retry, the empty line, or the groups. */
function InsideBody({ project, docRef, asked, level }: { project: string; docRef: string; asked: boolean; level: PartLevel }) {
  const t = useTranslate();
  const read = useInside(project, docRef, asked);
  const held = useHeldWorkspaces(project);
  const failure = useRetainedFailure(read.error, read.isFetching);
  const modules = held.data?.modules;
  const inside = useMemo(() => (read.data === undefined ? null : insideOf(read.data, insideDirections(modules ?? []))), [read.data, modules]);
  if (!asked) {
    return null;
  }
  if (read.data === undefined) {
    return failure === null ? (
      <div aria-busy="true">
        <Skeleton label={t("doc.inside.reading")} lines={3} />
      </div>
    ) : (
      <ErrorPanel
        title={t("doc.inside.failed")}
        message={apiErrorOf(failure).message}
        retrying={read.isFetching}
        attempt={read.errorUpdateCount}
        onRetry={() => {
          void read.refetch({ cancelRefetch: false });
        }}
      />
    );
  }
  if (read.data.reason !== null) {
    return (
      <p className="verbatim" lang="en">
        {read.data.reason}
      </p>
    );
  }
  if (inside === null || (inside.groups.length === 0 && inside.noId === 0)) {
    return <p className="muted">{t("doc.inside.none")}</p>;
  }
  return (
    <>
      {inside.groups.map((group) => (
        <GroupPart key={group.holder} project={project} group={group} level={level} />
      ))}
      {inside.noId > 0 && (
        <p className="doc-inside-noid">
          <a href={sectionHash(project, "tree", docRef)}>{t("doc.inside.noId", { count: inside.noId })}</a>
        </p>
      )}
    </>
  );
}

/** Inside: a disclosure at the part's level, held by the Shell; read once first expanded. */
export function DocInside({ project, docRef, open, onToggle, level }: { project: string; docRef: string; open: boolean; onToggle: (open: boolean) => void; level: PartLevel }) {
  const t = useTranslate();
  const [asked, setAsked] = useState(open);
  if (open && !asked) {
    setAsked(true);
  }
  return (
    <Disclosure summary={t("doc.inside.heading")} open={open} onToggle={onToggle} level={level} className="doc-part doc-inside">
      <InsideBody project={project} docRef={docRef} asked={asked} level={level} />
    </Disclosure>
  );
}
