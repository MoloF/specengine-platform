import type { NodeView, ShownLink, ShownNode } from "../api/types";
import { sectionHash } from "../app/routes";
import { useTranslate } from "../i18n";
import { splitFrontMatter } from "../markdown/source";
import { Facts } from "../tree/Facts";
import { isResolvedLink, linkStateLook } from "../tree/labels";
import { leftOutText } from "../tree/LinksPanel";
import { Badge } from "../ui/Badge";
import { CopyButton } from "../ui/CopyButton";
import { Disclosure } from "../ui/Disclosure";
import { Icon } from "../ui/Icon";

// What a person rarely needs and the AI reads (docs/features/doc-modal.md "Acceptance criteria",
// DM-12): folded, collapsed. The ID with Copy (an ID-less document: its path), the holder's facts,
// the norm hash, the front-matter as written, the links that do not resolve with the daemon's state
// and reason, what the live rule left out, the read's notes, and where the rest is.

/** The links that do not resolve, both ways, in served order. */
export function unresolvedOf(holder: ShownNode): ShownLink[] {
  return [...(holder.links?.outgoing ?? []), ...(holder.links?.incoming ?? [])].filter((link) => !isResolvedLink(link));
}

export function DocTechnical({ project, holder, read }: { project: string; holder: ShownNode; read: NodeView }) {
  const t = useTranslate();
  const name = holder.id ?? holder.path;
  const block = splitFrontMatter(holder.text).block;
  const unresolved = unresolvedOf(holder);
  const leftOut = holder.links === null ? null : leftOutText(holder.links.left_out, t);
  return (
    <Disclosure summary={t("ui.technicalDetails")} className="doc-technical">
      <dl className="facts">
        <div className="fact fact-wide">
          <dt>{holder.id === null ? t("tree.node.where") : t("doc.technical.id")}</dt>
          <dd className="doc-technical-id">
            <span className="mono">{name}</span> <CopyButton text={name} context={name} failed={t("doc.technical.copyFailed")} />
          </dd>
        </div>
      </dl>
      <Facts holder={holder} />
      <dl className="facts">
        <div className="fact fact-wide">
          <dt>{t("doc.technical.norm")}</dt>
          <dd className="mono">{holder.norm_hash ?? "-"}</dd>
        </div>
      </dl>
      {block !== null && (
        <>
          <p className="doc-subheading">{t("doc.technical.frontMatter")}</p>
          <pre className="md-front-matter doc-front-matter">{block}</pre>
        </>
      )}
      {unresolved.length > 0 && (
        <>
          <p className="doc-subheading">{t("doc.technical.unresolved", { count: unresolved.length })}</p>
          <ul className="doc-unresolved">
            {unresolved.map((link, index) => (
              <li key={`${String(index)}-${link.path}:${String(link.line)}`}>
                <span className="mono" lang="en">
                  {link.type} {link.written}
                </span>{" "}
                <Badge name={t("tree.links.state")} look={linkStateLook(link.state, t)} />
                {link.reason !== null && (
                  <span className="verbatim doc-unresolved-reason" lang="en">
                    {link.reason}
                  </span>
                )}
              </li>
            ))}
          </ul>
        </>
      )}
      {leftOut !== null && (
        <p className="note">
          <Icon name="info" />
          <span>{leftOut}</span>
        </p>
      )}
      {read.notes.length > 0 && (
        <ul className="note-list" aria-label={t("doc.technical.notes")}>
          {read.notes.map((note) => (
            <li key={note} lang="en">
              {note}
            </li>
          ))}
        </ul>
      )}
      <p className="muted">{t("doc.technical.inTree")}</p>
      <p>
        <a href={sectionHash(project, "graph", name)}>
          <Icon name="graph" />
          <span>{t("doc.openGraph")}</span>
        </a>
      </p>
    </Disclosure>
  );
}
