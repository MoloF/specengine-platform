import { Fragment, lazy, Suspense, useMemo } from "react";
import type { Cited } from "../api/types";
import { GUIDE_PREFIX } from "../app/routes";
import { DocLink } from "../doc/DocLink";
import { shownSlot, shownText } from "../ui/text";
import { citedEntries, codeSpans, runsOf, type CitedEntries } from "./cited";
import type { Shown } from "./GuideCitedText";

// A stored plain text (a card's title, goal, criteria, its items' texts and history notes; an Inbox
// item's one-line slots) shown as the CLI prints it (src/ui/text.ts), each occurrence of a string the
// daemon served in its `cited` a document link to the cited node (docs/features/department-practices.md
// "Acceptance criteria", DP21): outside a backtick code span alone, and nothing else linked. A
// `guide:<topic>[#<ID>]` citation of a topic SpecEngine's guide serves links to that topic's page
// (docs/features/specengine-guide.md "The UI"): only such a text reads the guide's index, and a topic
// it does not hold stays text. With no served list and no citation the text is exactly as before. The
// served strings are sorted once per served list, and a text is cut again only when it or the list
// changes.

/** The served citations of the text's holder, sorted for matching, and the project its links open in; null: nothing is linked. */
export interface CitedLinks {
  project: string;
  cited: readonly Cited[];
  entries: CitedEntries;
}

/** Each served list's links per project, built once: a read's answer keeps its list, so every text of it shares one. */
const built = new WeakMap<readonly Cited[], Map<string, CitedLinks>>();

/** No served citation: the guide's citations alone link. */
const NONE: readonly Cited[] = [];

/**
 * The served list as a text takes it, its project always: a read that served no list, or an empty
 * one, links the guide's citations alone.
 */
export function citedLinks(project: string, cited: readonly Cited[] | null): CitedLinks {
  const list = cited === null || cited.length === 0 ? NONE : cited;
  const byProject = built.get(list) ?? new Map<string, CitedLinks>();
  built.set(list, byProject);
  const known = byProject.get(project);
  if (known !== undefined) {
    return known;
  }
  const links = { project, cited: list, entries: citedEntries(list) };
  byProject.set(project, links);
  return links;
}

/** Keeps a text as given: a line a surface already draws unescaped. */
function asGiven(text: string): string {
  return text;
}

/**
 * A text holding `guide:`, a chunk of its own out of first load (docs/canon/ui-languages.md "Size"):
 * the text as before until it lands, then its guide citations linked.
 */
const GuideCitedText = lazy(() => import("./GuideCitedText").then((module) => ({ default: module.GuideCitedText })));

/** A text's served IDs as document links. */
function IdCitedText({ text, links, shown }: { text: string; links: CitedLinks; shown: Shown }) {
  const runs = useMemo(() => runsOf(text, links.entries, codeSpans(text)), [text, links]);
  return runs.map((run, index) =>
    run.name === null ? (
      <Fragment key={index}>{shown(run.text)}</Fragment>
    ) : (
      <DocLink key={index} className="cited-link" project={links.project} docRef={run.name}>
        {shown(run.text)}
      </DocLink>
    ),
  );
}

/** A text verbatim (`slot`: on one line; `raw`: as given, as its surface drew it before), its served citations as document links, its guide citations as the guide's. */
export function CitedText({ text, links, slot = false, raw = false }: { text: string; links: CitedLinks | null; slot?: boolean; raw?: boolean }) {
  const shown = raw ? asGiven : slot ? shownSlot : shownText;
  if (links === null) {
    return shown(text);
  }
  if (text.includes(GUIDE_PREFIX)) {
    return (
      <Suspense fallback={shown(text)}>
        <GuideCitedText text={text} links={links} shown={shown} />
      </Suspense>
    );
  }
  return links.entries.length === 0 ? shown(text) : <IdCitedText text={text} links={links} shown={shown} />;
}
