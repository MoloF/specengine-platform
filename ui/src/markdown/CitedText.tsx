import { Fragment, useMemo } from "react";
import type { Cited } from "../api/types";
import { DocLink } from "../doc/DocLink";
import { shownSlot, shownText } from "../ui/text";
import { citedEntries, codeSpans, runsOf, type CitedEntries } from "./cited";

// A stored plain text (a card's title, goal, criteria, its items' texts and history notes; an Inbox
// item's one-line slots) shown as the CLI prints it (src/ui/text.ts), each occurrence of a string the
// daemon served in its `cited` a document link to the cited node (docs/features/department-practices.md
// "Acceptance criteria", DP21): outside a backtick code span alone, and nothing else linked. With no
// served list the text is exactly as before. The served strings are sorted once per served list, and a
// text is cut again only when it or the list changes.

/** The served citations of the text's holder, sorted for matching, and the project its links open in; null: nothing is linked. */
export interface CitedLinks {
  project: string;
  cited: readonly Cited[];
  entries: CitedEntries;
}

/** Each served list's links per project, built once: a read's answer keeps its list, so every text of it shares one. */
const built = new WeakMap<readonly Cited[], Map<string, CitedLinks>>();

/** The served list as a text takes it: none when the read did not serve one, or served none. */
export function citedLinks(project: string, cited: readonly Cited[] | null): CitedLinks | null {
  if (cited === null || cited.length === 0) {
    return null;
  }
  const byProject = built.get(cited) ?? new Map<string, CitedLinks>();
  built.set(cited, byProject);
  const known = byProject.get(project);
  if (known !== undefined) {
    return known;
  }
  const links = { project, cited, entries: citedEntries(cited) };
  byProject.set(project, links);
  return links;
}

/** Keeps a text as given: a line a surface already draws unescaped. */
function asGiven(text: string): string {
  return text;
}

/** A text verbatim (`slot`: on one line; `raw`: as given, as its surface drew it before), its served citations as document links. */
export function CitedText({ text, links, slot = false, raw = false }: { text: string; links: CitedLinks | null; slot?: boolean; raw?: boolean }) {
  const shown = raw ? asGiven : slot ? shownSlot : shownText;
  const runs = useMemo(() => (links === null ? null : runsOf(text, links.entries, codeSpans(text))), [text, links]);
  if (links === null || runs === null) {
    return shown(text);
  }
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
