import { Fragment, useMemo, type ReactNode } from "react";
import { useGuide } from "../api/queries";
import { GUIDE_PREFIX, guideHash } from "../app/routes";
import { DocLink } from "../doc/DocLink";
import { codeSpans } from "./cited";
import type { CitedLinks } from "./CitedText";
import { linkRuns } from "./guide";

// A plain text holding `guide:` (./CitedText), a chunk of its own: only such a text reads SpecEngine's
// guide index, and each `guide:<topic>[#<ID>]` citation of a topic the index serves, outside a code
// span, links to that topic's page at the rule (docs/features/specengine-guide.md "The UI"); a topic it
// does not hold, a citation in code and any while the index is out or failed stay text. The served IDs
// between citations link as ./CitedText links them.

/** How a surface shows a stored text's run (src/ui/text.ts). */
export type Shown = (text: string) => ReactNode;

/** A text holding `guide:`: the guide's index read, its citations of served topics linked with the served IDs. */
export function GuideCitedText({ text, links, shown }: { text: string; links: CitedLinks; shown: Shown }) {
  const topics = useGuide({ citing: true }).data?.topics;
  const runs = useMemo(() => {
    const cited = topics === undefined ? [] : topics.map((entry) => entry.topic).filter((topic) => topic !== "" && text.includes(`${GUIDE_PREFIX}${topic}`));
    return linkRuns(text, links.entries, cited, codeSpans(text));
  }, [text, links, topics]);
  return runs.map((run, index) => {
    if (run.guide !== null) {
      return (
        <a key={index} className="cited-link" href={guideHash(links.project, run.guide.topic, run.guide.rule)}>
          {shown(run.text)}
        </a>
      );
    }
    return run.name === null ? (
      <Fragment key={index}>{shown(run.text)}</Fragment>
    ) : (
      <DocLink key={index} className="cited-link" project={links.project} docRef={run.name}>
        {shown(run.text)}
      </DocLink>
    );
  });
}
