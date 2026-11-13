import { useId, useRef, useState } from "react";
import { useRings } from "../api/queries";
import type { Ring } from "../api/types";
import { rollupLook } from "../flow/labels";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { EmptyState } from "../ui/EmptyState";
import { Icon } from "../ui/Icon";
import { ReadFailure, Skeleton } from "../ui/states";
import { TabPanel, Tabs } from "../ui/Tabs";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { RingsList } from "./RingsList";
import { RingsPicture } from "./RingsPicture";
import { statesOf } from "./view";

// The Graph's Rings view (docs/features/scope-rings.md "Acceptance criteria", SR7): the project's
// nodes on concentric rings, one per `[[map.ring]]`, the first in the middle, read from the daemon
// and drawn as served. A search and the state chips dim what they leave out, in the picture and the
// List alike. No ring: the next step and an example; the config's notes, when it has some, verbatim.
// A chunk of its own, reached from graph/GraphHome.tsx by a dynamic import alone (SR8).

/** The config's words the next step names: code, the same in every language. */
const RING_TABLE = "[[map.ring]]";
const CONFIG_FILE = "specengine.toml";
const KEY_TITLE = "title";
const KEY_ROOT = "root";
const KEY_KINDS = "kinds";

/** A ring as the config writes one, its values left for the project to fill. */
const RING_EXAMPLE = `${RING_TABLE}\n${KEY_TITLE} = "<title>"\n${KEY_ROOT} = "<REF>"\n${KEY_KINDS} = ["<kind>"]`;

type RingsTab = "picture" | "list";

/** How to add a ring: the next step, then the example in mono. */
function NextStep() {
  const t = useTranslate();
  return (
    <>
      <p>
        {fill(t("rings.next", { table: slot("table"), file: slot("file"), title: slot("title"), root: slot("root"), kinds: slot("kinds") }), {
          table: <code>{RING_TABLE}</code>,
          file: <code>{CONFIG_FILE}</code>,
          title: <code>{KEY_TITLE}</code>,
          root: <code>{KEY_ROOT}</code>,
          kinds: <code>{KEY_KINDS}</code>,
        })}
      </p>
      <pre className="mono rings-example">{RING_EXAMPLE}</pre>
    </>
  );
}

/** The config's notes, the daemon's words verbatim. */
function Notes({ notes }: { notes: readonly string[] }) {
  const t = useTranslate();
  return (
    <ul className="note-list rings-notes" aria-label={t("rings.notes")}>
      {notes.map((note, index) => (
        <li key={`${String(index)}-${note}`} lang="en">
          {note}
        </li>
      ))}
    </ul>
  );
}

/** The served rings: the search and the chips over both views, then Picture and List. */
function Rings({ project, rings }: { project: string; rings: readonly Ring[] }) {
  const t = useTranslate();
  const fieldId = useId();
  const tabsBase = useId();
  const [query, setQuery] = useState("");
  const [released, setReleased] = useState<ReadonlySet<string>>(() => new Set());
  const [tab, setTab] = useState<RingsTab>("picture");
  return (
    <>
      <div className="rings-controls">
        <div className="rings-search">
          <label className="field-label" htmlFor={fieldId}>
            {t("rings.search")}
          </label>
          <input
            id={fieldId}
            className="field"
            type="search"
            autoComplete="off"
            spellCheck={false}
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
            }}
          />
        </div>
        <div className="rings-chips" role="group" aria-label={t("rings.states")}>
          {statesOf(rings).map(({ state, count }) => {
            const look = rollupLook(state, t);
            const pressed = !released.has(state);
            return (
              <button
                key={state}
                type="button"
                className={`chip rings-chip tone-${look.tone}`}
                aria-pressed={pressed}
                onClick={() => {
                  setReleased((current) => {
                    const next = new Set(current);
                    if (!next.delete(state)) {
                      next.add(state);
                    }
                    return next;
                  });
                }}
              >
                <span className="chip-check" aria-hidden="true">
                  {pressed && <Icon name="selected" />}
                </span>
                <Icon name={look.icon} />
                <span lang={look.served === true ? "en" : undefined}>{look.label}</span> <span className="chip-count">{count}</span>
              </button>
            );
          })}
        </div>
      </div>
      <Tabs
        label={t("rings.views")}
        base={tabsBase}
        selected={tab}
        onSelect={setTab}
        tabs={[
          { id: "picture", label: t("rings.picture") },
          { id: "list", label: t("rings.list") },
        ]}
      />
      <TabPanel base={tabsBase} id="picture" selected={tab === "picture"}>
        {tab === "picture" && <RingsPicture rings={rings} query={query} released={released} />}
      </TabPanel>
      <TabPanel base={tabsBase} id="list" selected={tab === "list"}>
        {tab === "list" && <RingsList project={project} rings={rings} query={query} released={released} />}
      </TabPanel>
    </>
  );
}

export function RingsView({ project }: { project: string }) {
  const t = useTranslate();
  const headingId = useId();
  const region = useRef<HTMLElement>(null);
  const read = useRings(project);
  const failure = useRetainedFailure(read.error, read.isFetching);
  const retried = useRetryFocus(read.data, () => region.current);
  const answer = read.data;

  let body;
  if (answer === undefined) {
    body =
      failure === null ? (
        <Skeleton label={t("rings.loading")} lines={6} />
      ) : (
        <ReadFailure
          title={t("rings.failed")}
          failure={failure}
          retrying={read.isFetching}
          attempt={read.errorUpdateCount}
          onRetry={() => {
            retried();
            void read.refetch({ cancelRefetch: false });
          }}
        />
      );
  } else if (answer.rings.length === 0) {
    body =
      answer.notes.length === 0 ? (
        <EmptyState level={3} title={t("rings.none")} description={<NextStep />} />
      ) : (
        <div className="rings-config-notes">
          <Notes notes={answer.notes} />
          <NextStep />
        </div>
      );
  } else {
    body = (
      <>
        {answer.notes.length > 0 && <Notes notes={answer.notes} />}
        <Rings project={project} rings={answer.rings} />
      </>
    );
  }

  return (
    <section ref={region} tabIndex={-1} className="rings-view" aria-labelledby={headingId} aria-busy={read.isFetching}>
      <h2 id={headingId} className="rings-heading">
        {t("rings.title")}
      </h2>
      {body}
    </section>
  );
}
