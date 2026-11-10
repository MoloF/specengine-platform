import { useId } from "react";
import type { ShownLink, ShownLinks } from "../api/types";
import { useTranslate } from "../i18n";
import { isResolvedLink, isWeakLink } from "../tree/labels";
import { Disclosure } from "../ui/Disclosure";
import { Lozenge } from "../ui/Lozenge";
import { DocLink } from "./DocLink";

// A document's links by title (docs/features/doc-modal.md "Acceptance criteria", DM-12): out, then
// in, each the served strong links that resolve, in served order, a row its raw type (ADR-0031,
// never translated or branched on) and the other end's title, else its name, as a document link; the
// weak ones folded under Mentions. A link that does not resolve stands under Technical details alone.

/** The heading levels a part takes: its own, one below the frame's title. */
export type PartLevel = 2 | 3;

/** A part's heading at its level. */
export function PartHeading({ level, id, children }: { level: PartLevel; id?: string; children: string }) {
  return level === 2 ? (
    <h2 id={id} className="issue-section-title">
      {children}
    </h2>
  ) : (
    <h3 id={id} className="issue-section-title">
      {children}
    </h3>
  );
}

/** One link: the type as served, then the end by its title. */
function LinkRow({ project, link }: { project: string; link: ShownLink & { name: string } }) {
  return (
    <li className="doc-link-row">
      <Lozenge appearance="neutral" icon="linkResolved" label={link.type} lang="en" />
      <DocLink project={project} docRef={link.name} className="doc-link-end">
        <bdi>{link.title ?? link.name}</bdi>
      </DocLink>
    </li>
  );
}

/** Out or In: a group named by its line, its rows in served order. */
function LinkList({ project, title, links }: { project: string; title: string; links: readonly (ShownLink & { name: string })[] }) {
  const id = useId();
  return (
    <div className="doc-link-group" role="group" aria-labelledby={id}>
      <p id={id} className="doc-subheading">
        {title}
      </p>
      <ul className="doc-link-list">
        {links.map((link, index) => (
          <LinkRow key={`${String(index)}-${link.path}:${String(link.line)}`} project={project} link={link} />
        ))}
      </ul>
    </div>
  );
}

/** The links part: Out, In, Mentions folded, or none at all. */
export function DocLinks({ project, links, level }: { project: string; links: ShownLinks | null; level: PartLevel }) {
  const t = useTranslate();
  const id = useId();
  const strong = (link: ShownLink): link is ShownLink & { name: string } => isResolvedLink(link) && !isWeakLink(link);
  const out = links?.outgoing.filter(strong) ?? [];
  const into = links?.incoming.filter(strong) ?? [];
  const mentions = [...(links?.outgoing ?? []), ...(links?.incoming ?? [])].filter((link): link is ShownLink & { name: string } => isResolvedLink(link) && isWeakLink(link));
  const none = out.length === 0 && into.length === 0 && mentions.length === 0;
  return (
    <section className="doc-part doc-links" aria-labelledby={id}>
      <PartHeading level={level} id={id}>
        {t("doc.links.heading")}
      </PartHeading>
      {none ? (
        <p className="muted">{t("doc.links.none")}</p>
      ) : (
        <>
          {out.length > 0 && <LinkList project={project} title={t("doc.links.out")} links={out} />}
          {into.length > 0 && <LinkList project={project} title={t("doc.links.in")} links={into} />}
          {mentions.length > 0 && (
            <Disclosure summary={t("doc.links.mentions", { count: mentions.length })} className="doc-mentions">
              <ul className="doc-link-list">
                {mentions.map((link, index) => (
                  <LinkRow key={`${String(index)}-${link.path}:${String(link.line)}`} project={project} link={link} />
                ))}
              </ul>
            </Disclosure>
          )}
        </>
      )}
    </section>
  );
}
