import { useEffect, useId, useMemo, useRef, useState } from "react";
import { useHeldWorkspaces } from "../api/queries";
import type { LinkUsage, ShownLink, ShownLinks } from "../api/types";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { isResolvedLink, isWeakLink } from "../tree/labels";
import { Button } from "../ui/Button";
import { Disclosure } from "../ui/Disclosure";
import { Lozenge } from "../ui/Lozenge";
import { DocLink } from "./DocLink";
import { insideDirections, type KindSet } from "./inside";
import { USED_BY_SHOWN, usageLayout, type UsageUser } from "./usage";

// A document's links by title (docs/features/doc-modal.md "Acceptance criteria", DM-12): out, then
// in, each the served strong links that resolve, a row its raw type (ADR-0031, never translated or
// branched on) and the other end's title, else its name, as a document link; the weak ones folded
// under Mentions. A link that does not resolve stands under Technical details alone. In says who
// uses the document (docs/features/usage-orphans.md "Acceptance criteria", UO10): the daemon's
// counts, then its users by direction and kind with every place each cites from, or, where the
// daemon counts none, that nothing uses it yet; the rows of its own file apart, outside the count.

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

/** Out: a group named by its line, its rows in served order. */
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

/** One user: its title (else name) as a document link, then each place it cites from: `path:line`, the link's raw type, the section it lands on. */
function UserRow({ project, user }: { project: string; user: UsageUser }) {
  const t = useTranslate();
  return (
    <li className="doc-usage-user">
      <DocLink project={project} docRef={user.name} className="doc-usage-link">
        <bdi>{user.title ?? user.name}</bdi>
      </DocLink>
      <ul className="doc-usage-places">
        {user.places.map((place, index) => (
          <li key={`${String(index)}-${place.path}:${String(place.line)}`} className="doc-usage-place">
            <span className="mono">
              {place.path}:{place.line}
            </span>{" "}
            <Lozenge appearance="neutral" icon="linkResolved" label={place.type} lang="en" />
            {place.at !== null && (
              <>
                {" "}
                {fill(t("tree.links.at", { at: slot("at") }), { at: <span className="mono">{place.at}</span> })}
              </>
            )}
          </li>
        ))}
      </ul>
    </li>
  );
}

/** A group's label: a direction's title in the project's words, a raw kind under `lang="en"`, none the chrome's; then its count of users. */
function GroupLabel({ id, set, count }: { id: string; set: KindSet<UsageUser> | null; count: number | null }) {
  const t = useTranslate();
  let label;
  if (set === null) {
    label = t("doc.links.sameDocument");
  } else if (set.by === "direction") {
    label = <bdi>{set.title}</bdi>;
  } else if (set.kind === null) {
    label = t("doc.inside.noKind");
  } else {
    label = (
      <bdi lang="en">
        {set.kind}
      </bdi>
    );
  }
  return (
    <p id={id} className="doc-subheading doc-usage-label">
      {label}
      {count !== null && <> · {count}</>}
    </p>
  );
}

/** One group of users: the first USED_BY_SHOWN, then Show all, which takes focus to the first it shows. */
function UserGroup({ project, set, users }: { project: string; set: KindSet<UsageUser> | null; users: readonly UsageUser[] }) {
  const t = useTranslate();
  const id = useId();
  const [all, setAll] = useState(false);
  const list = useRef<HTMLUListElement>(null);
  useEffect(() => {
    if (all) {
      Array.from(list.current?.querySelectorAll<HTMLElement>(".doc-usage-link") ?? [])[USED_BY_SHOWN]?.focus();
    }
  }, [all]);
  const shown = all ? users : users.slice(0, USED_BY_SHOWN);
  return (
    <div className="doc-usage-group" role="group" aria-labelledby={id}>
      <GroupLabel id={id} set={set} count={set === null ? null : users.length} />
      <ul ref={list} className="doc-usage-list">
        {shown.map((user) => (
          <UserRow key={user.name} project={project} user={user} />
        ))}
      </ul>
      {!all && users.length > USED_BY_SHOWN && (
        <Button
          appearance="subtle"
          spacing="compact"
          onClick={() => {
            setAll(true);
          }}
        >
          {t("doc.inside.showAll", { count: users.length })}
        </Button>
      )}
    </div>
  );
}

/**
 * In: the daemon's counts, then its users grouped, or that nothing uses it yet where the daemon
 * counts no use, whatever rows stand; the rows of the holder's own file last, apart.
 */
function UsedBy({ project, links, usage, path }: { project: string; links: ShownLinks; usage: LinkUsage; path: string }) {
  const t = useTranslate();
  const id = useId();
  const held = useHeldWorkspaces(project);
  const modules = held.data?.modules;
  const layout = useMemo(() => usageLayout(links, path, insideDirections(modules ?? [])), [links, path, modules]);
  return (
    <div className="doc-link-group doc-usage" role="group" aria-labelledby={id}>
      <p id={id} className="doc-subheading">
        {t("doc.links.in")}
      </p>
      <p className="doc-usage-counts">{t("doc.links.usage", { used: usage.used_by, mentioned: usage.mentioned_by })}</p>
      {usage.used_by === 0 ? (
        <p className="doc-usage-unused">{t("doc.links.unused")}</p>
      ) : (
        layout.sets.map((set) => <UserGroup key={set.by === "direction" ? `direction:${set.name}` : `kind:${set.kind ?? ""}`} project={project} set={set} users={set.entries} />)
      )}
      {layout.within.length > 0 && <UserGroup project={project} set={null} users={layout.within} />}
    </div>
  );
}

/** The links part: Out, In with who uses the document, Mentions folded; none at all said so, and that nothing uses it where the daemon counts none. */
export function DocLinks({ project, links, path, level }: { project: string; links: ShownLinks | null; path: string; level: PartLevel }) {
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
        <>
          <p className="muted">{t("doc.links.none")}</p>
          {links !== null && links.usage.used_by === 0 && <p className="doc-usage-unused">{t("doc.links.unused")}</p>}
        </>
      ) : (
        <>
          {out.length > 0 && <LinkList project={project} title={t("doc.links.out")} links={out} />}
          {links !== null && <UsedBy project={project} links={links} usage={links.usage} path={path} />}
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
