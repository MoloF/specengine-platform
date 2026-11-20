import { createContext, createElement, memo, use, useEffect, useMemo, useRef, type JSX, type MouseEvent, type ReactElement, type ReactNode } from "react";
import ReactMarkdown, { type Components, type ExtraProps } from "react-markdown";
import remarkGfm from "remark-gfm";
import { useGuide } from "../api/queries";
import type { Cited, ShownLink } from "../api/types";
import { GUIDE_PREFIX, guideHash, sectionHash } from "../app/routes";
import { DocLink } from "../doc/DocLink";
import { linkStateLook } from "../tree/labels";
import type { LineTarget } from "../tree/TextView";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { numberProperty, remarkSpec, stringProperty, type HastElement, type SpecWords } from "./plugin";
import { splitFrontMatter } from "./source";
import { useLanguage, useTranslate, type Translate } from "../i18n";

// The renderer (docs/features/ui-markdown.md; ADR-0036), loaded on demand like the graph canvas:
// the only module importing react-markdown and remark-gfm. React elements only, never an HTML
// string; raw HTML is text (./plugin); every component below writes the attributes it names and
// never spreads what it is given. An anchor exists only where the node's links read lists the
// link as resolved: a document link (src/doc/DocLink.tsx) everywhere, the Spec tree's own link in the
// tree (docs/features/doc-modal.md "Acceptance criteria", DM-16).

export interface MarkdownProps {
  /** The text, exactly as the daemon sent it: never trimmed or normalised. */
  source: string;
  /** The holder's outgoing links from its links read; null before the read lands or outside the Node tab. */
  links: readonly ShownLink[] | null;
  project: string;
  /** The level of the heading the text sits under: its own headings start one below. */
  baseLevel: 1 | 2 | 3 | 4 | 5;
  /** A document holder's text: a leading front-matter block is shown verbatim. */
  frontMatter: boolean;
  /** The file line of the text's first line (default 1): `data-line` counts from it. */
  firstLine?: number;
  /** The file the text is in: a link of the links read matches only at this path. */
  path?: string | null;
  /** A jump: the block holding this line is focused. */
  target?: LineTarget | null;
  /** An anchor to another node was clicked (the Spec tree's links alone). */
  onFollow?: (event: MouseEvent<HTMLAnchorElement>) => void;
  /** Where an anchor goes: the document view (the default), or, in the Spec tree, the tree's node. */
  linkTo?: "doc" | "tree";
  /** The title the frame shows: a first heading saying the same is left out. */
  dropTitle?: string | null;
  /** The first top-level paragraph alone, no front matter: a panel's lead; none, nothing. */
  firstParagraph?: boolean;
  /** Classes of the text's container. */
  className?: string;
  /**
   * The IDs the daemon resolved in the holder's texts (a card's, a proposal's: its `cited`): each
   * served `written` outside code a document link to its `name`, nothing else
   * (docs/features/department-practices.md "Acceptance criteria", DP21). Null or absent: none.
   */
  cited?: readonly Cited[] | null;
  /**
   * Headings made anchors (docs/features/specengine-guide.md "The UI": a guide page's rules): each
   * heading whose `{#ID}` is listed takes the ID as its `id`, `tool` drawn beside it. Null or absent:
   * no heading is one.
   */
  anchors?: HeadingAnchors | null;
}

/** The headings of a text that are anchors, what stands beside each, and the one to bring into view. */
export interface HeadingAnchors {
  /** The `{#ID}`s that are anchors: only these. */
  ids: readonly string[];
  /** Drawn beside the anchor's heading, after it. */
  tool: (id: string) => ReactNode;
  /** The anchor the holder asks for; null: none. */
  wanted: string | null;
  /** Called with the wanted anchor's heading once it is drawn, and again when it is asked anew. */
  onReach: (heading: HTMLElement) => void;
}

interface LinkScope {
  project: string;
  links: readonly ShownLink[] | null;
  path: string | null;
  linkTo: "doc" | "tree";
  onFollow: ((event: MouseEvent<HTMLAnchorElement>) => void) | undefined;
}

// The links read reaches the anchors through context: its landing re-renders them, never the parse.
const LinkScopeContext = createContext<LinkScope>({ project: "", links: null, path: null, linkTo: "doc", onFollow: undefined });

// The anchors reach the headings through context too: a holder's tool or wanted anchor never parses again.
const AnchorContext = createContext<HeadingAnchors | null>(null);

type Props<Tag extends keyof JSX.IntrinsicElements> = JSX.IntrinsicElements[Tag] & ExtraProps;

/**
 * The links read's entries for a link written here: outgoing inline links at the same place and
 * bytes. More than one when the destination also reads as an ID (`[R-9](R-9)`: a mention and a
 * file link, both `inline`).
 */
function listed(scope: LinkScope, written: string | null, line: number | null): ShownLink[] {
  if (scope.links === null || written === null || line === null) {
    return [];
  }
  return scope.links.filter((link) => link.origin === "inline" && link.path === scope.path && link.line === line && link.written === written);
}

/** The node every entry resolved to, the same one; null when any is unresolved or they disagree. */
function resolvedName(entries: readonly ShownLink[]): string | null {
  const name = entries[0]?.name ?? null;
  return name !== null && entries.every((entry) => entry.state === "resolved" && entry.name === name) ? name : null;
}

function Anchor({ node, children }: Props<"a">) {
  const t = useTranslate();
  const scope = use(LinkScopeContext);
  const kind = stringProperty(node, "dataKind");
  if (kind === "external") {
    return (
      <span className="md-link md-link-external">
        {children} <span className="md-url">{stringProperty(node, "dataUrl")}</span>
      </span>
    );
  }
  if (kind === "autolink") {
    return <span className="md-url">{children}</span>;
  }
  const topic = kind === "guide" ? stringProperty(node, "dataTopic") : null;
  if (topic !== null) {
    // A citation of the guide whose topic the served index holds (./plugin, ./guide): its page, at the rule.
    return (
      <a className="md-link" href={guideHash(scope.project, topic, stringProperty(node, "dataRule"))}>
        {children}
      </a>
    );
  }
  const citedName = kind === "cited" ? stringProperty(node, "dataName") : null;
  if (citedName !== null) {
    // A served citation: the document view at the node the daemon resolved (./plugin, ./cited).
    return (
      <DocLink className="md-link" project={scope.project} docRef={citedName}>
        {children}
      </DocLink>
    );
  }
  const entries = kind === "local" ? listed(scope, stringProperty(node, "dataDest"), numberProperty(node, "dataDestLine")) : [];
  const name = resolvedName(entries);
  if (name !== null) {
    return scope.linkTo === "tree" ? (
      <a className="md-link" href={sectionHash(scope.project, "tree", name)} onClick={scope.onFollow}>
        {children}
      </a>
    ) : (
      <DocLink className="md-link" project={scope.project} docRef={name}>
        {children}
      </DocLink>
    );
  }
  const unresolved = entries.find((entry) => entry.state !== "resolved");
  if (unresolved === undefined) {
    return <span className="md-link">{children}</span>;
  }
  return (
    <span className="md-link md-link-unresolved">
      {children} <Badge name={t("markdown.link")} look={linkStateLook(unresolved.state, t)} />
    </span>
  );
}

/**
 * A heading its holder made an anchor: the ID its `id`, focusable from script alone, the holder's tool
 * beside it; handed to the holder once drawn when it is the wanted one.
 */
function AnchorHeading({ level, id, line, anchors, children }: { level: number; id: string; line: number | null; anchors: HeadingAnchors; children: ReactNode }) {
  const ref = useRef<HTMLHeadingElement>(null);
  const wanted = anchors.wanted === id;
  const { onReach } = anchors;
  useEffect(() => {
    if (wanted && ref.current !== null) {
      onReach(ref.current);
    }
  }, [wanted, onReach]);
  return (
    <div className="md-anchor" data-line={line ?? undefined}>
      {createElement(`h${String(level)}`, { ref, id, tabIndex: -1, className: "md-heading" }, children)}
      {anchors.tool(id)}
    </div>
  );
}

function heading(level: 1 | 2 | 3 | 4 | 5 | 6) {
  return function Heading({ node, children }: Props<"h1">) {
    const anchors = use(AnchorContext);
    const id = stringProperty(node, "dataAnchor");
    const line = numberProperty(node, "dataLine");
    if (anchors !== null && id !== null && anchors.ids.includes(id)) {
      return (
        <AnchorHeading level={level} id={id} line={line} anchors={anchors}>
          {children}
        </AnchorHeading>
      );
    }
    return createElement(`h${String(level)}`, { className: "md-heading", "data-line": line ?? undefined }, children);
  };
}

/** An image is never loaded: its alt text and path as text (./plugin turns each into that first). */
function Image({ alt }: Props<"img">) {
  const t = useTranslate();
  return <span className="md-image">{alt === undefined || alt === "" ? t("markdown.image") : t("markdown.imageAlt", { alt })}</span>;
}

/** A task item's box: an icon and a word, never a form control. */
function TaskState({ checked }: Props<"input">) {
  const t = useTranslate();
  const done = checked === true;
  return (
    <span className="md-task-state">
      <Icon name={done ? "taskItemDone" : "taskItemOpen"} />
      <span>{done ? t("markdown.done") : t("markdown.notDone")}</span>
    </span>
  );
}

function Code({ children }: Props<"code">) {
  return <code>{children}</code>;
}

/** A code block: verbatim, wrapping, its info string as a label. */
function Pre({ node, children }: Props<"pre">) {
  const code = node?.children[0];
  const inner: HastElement | undefined = code?.type === "element" ? code : undefined;
  const info = stringProperty(inner, "dataInfo");
  return (
    <figure className="md-code" data-line={numberProperty(inner, "dataLine") ?? undefined}>
      {info !== null && info !== "" && <figcaption className="md-code-info">{info}</figcaption>}
      <pre className="md-code-block">{children}</pre>
    </figure>
  );
}

/** A table: its own focusable scroll region, named by its line. */
function Table({ node, children }: Props<"table">) {
  const t = useTranslate();
  const line = numberProperty(node, "dataLine");
  return (
    <div className="md-table-region" role="region" aria-label={line === null ? t("markdown.table") : t("markdown.tableAt", { line })} tabIndex={0} data-line={line ?? undefined}>
      <table className="md-table">{children}</table>
    </div>
  );
}

const COMPONENTS: Components = {
  a: Anchor,
  img: Image,
  input: TaskState,
  code: Code,
  pre: Pre,
  table: Table,
  h1: heading(1),
  h2: heading(2),
  h3: heading(3),
  h4: heading(4),
  h5: heading(5),
  h6: heading(6),
};

/** How many parses are kept, the last used last: a screen's texts and a few more. */
const PARSES_KEPT = 32;

/**
 * The last parses by text, lines and level. A render React makes again before committing it (one a
 * later read's answer cut into, or Strict Mode's second call) finds the elements the first one made,
 * so a text is parsed once per change however often it is rendered (docs/features/ui-markdown.md
 * AC-13). The elements carry no state: the links reach them through context, the focus through the
 * DOM.
 */
const parses = new Map<string, ReactElement>();

/** The words the rewrite writes, in the language `t` reads. */
function specWords(t: Translate): SpecWords {
  return {
    image: t("markdown.image"),
    imageAlt: (alt) => t("markdown.imageAlt", { alt }),
    footnote: (label) => t("markdown.footnote", { label }),
  };
}

/** What a parse keeps of a text: all of it, less a title said again, or its first paragraph alone; the served citations and guide topics it links. */
interface Cut {
  dropTitle: string | null;
  firstParagraph: boolean;
  cited: readonly Cited[];
  guideTopics: readonly string[];
}

/** The text's elements: react-markdown's sync component called once per text, lines, level, cut and language. */
function parsed(body: string, lineBase: number, baseLevel: number, cut: Cut, language: string, words: SpecWords): ReactElement {
  const key = `${language}:${String(lineBase)}:${String(baseLevel)}:${cut.firstParagraph ? "lead" : "all"}:${JSON.stringify(cut.dropTitle)}:${JSON.stringify(cut.cited.map((entry) => [entry.written, entry.name]))}:${JSON.stringify(cut.guideTopics)}:${body}`;
  const kept = parses.get(key);
  if (kept !== undefined) {
    parses.delete(key);
    parses.set(key, kept);
    return kept;
  }
  const elements = ReactMarkdown({ remarkPlugins: [remarkGfm, [remarkSpec, { source: body, lineBase, baseLevel, words, ...cut }]], components: COMPONENTS, children: body });
  parses.set(key, elements);
  for (const oldest of parses.keys()) {
    if (parses.size <= PARSES_KEPT) {
      break;
    }
    parses.delete(oldest);
  }
  return elements;
}

/** The parse and its elements: again only when the text, its lines, its level, its cut, its citations or the language change. */
const Rendered = memo(function Rendered({
  body,
  lineBase,
  baseLevel,
  dropTitle,
  firstParagraph,
  cited,
  guideTopics,
  language,
  words,
}: {
  body: string;
  lineBase: number;
  baseLevel: number;
  dropTitle: string | null;
  firstParagraph: boolean;
  cited: readonly Cited[];
  guideTopics: readonly string[];
  language: string;
  words: SpecWords;
}) {
  return parsed(body, lineBase, baseLevel, { dropTitle, firstParagraph, cited, guideTopics }, language, words);
});

/** No served citation. */
const NONE_CITED: readonly Cited[] = [];

/** No guide topic. */
const NO_TOPICS: readonly string[] = [];

type RenderedProps = Parameters<typeof Rendered>[0];

/**
 * A text holding `guide:` (docs/features/specengine-guide.md "The UI"): parsed with the guide's topics
 * it cites, as the served index lists them; none while the index is out or when it failed. Only such a
 * text reads the index.
 */
function GuideRendered(props: Omit<RenderedProps, "guideTopics">) {
  const { body } = props;
  const topics = useGuide({ citing: true }).data?.topics;
  const guideTopics = useMemo(() => (topics === undefined ? NO_TOPICS : topics.map((entry) => entry.topic).filter((topic) => topic !== "" && body.includes(`${GUIDE_PREFIX}${topic}`))), [topics, body]);
  return <Rendered {...props} guideTopics={guideTopics} />;
}

/** Markdown prose as React elements; a document's front-matter verbatim above it, but in a lead. */
export function Markdown({
  source,
  links,
  project,
  baseLevel,
  frontMatter,
  firstLine = 1,
  path = null,
  target = null,
  onFollow,
  linkTo = "doc",
  dropTitle = null,
  firstParagraph = false,
  className,
  cited = null,
  anchors = null,
}: MarkdownProps) {
  const t = useTranslate();
  const { language } = useLanguage();
  const words = useMemo(() => specWords(t), [t]);
  const container = useRef<HTMLDivElement>(null);
  const split = useMemo(() => (frontMatter ? splitFrontMatter(source) : { block: null, lines: 0, body: source }), [source, frontMatter]);
  // A leading BOM with no front-matter: the parser skips it without counting it in its offsets, so
  // it is dropped here (a line of its own it is not) and every offset stays the text's.
  const body = split.block === null && split.body.startsWith("\uFEFF") ? split.body.slice(1) : split.body;
  // The served citations this text holds: the parse's key names only these, so a list with others leaves its parse alone.
  const held = useMemo(() => (cited === null ? NONE_CITED : cited.filter((entry) => entry.written !== "" && body.includes(entry.written))), [cited, body]);
  const scope = useMemo<LinkScope>(() => ({ project, links, path, linkTo, onFollow }), [project, links, path, linkTo, onFollow]);

  // A jump focuses the last block starting at or before the line: the one holding it.
  useEffect(() => {
    const root = container.current;
    if (target === null || root === null) {
      return;
    }
    let found: HTMLElement | null = null;
    for (const block of root.querySelectorAll<HTMLElement>("[data-line]")) {
      if (Number(block.dataset.line) <= target.line) {
        found = block;
      }
    }
    for (const previous of root.querySelectorAll("[data-target]")) {
      previous.removeAttribute("data-target");
    }
    if (found === null) {
      return;
    }
    found.setAttribute("data-target", "");
    if (!found.hasAttribute("tabindex")) {
      found.setAttribute("tabindex", "-1");
    }
    found.scrollIntoView({ block: "center" });
    found.focus();
  }, [target]);

  return (
    <LinkScopeContext value={scope}>
      <AnchorContext value={anchors}>
        <div ref={container} className={className === undefined ? "markdown" : `markdown ${className}`}>
          {split.block !== null && !firstParagraph && (
            <pre className="md-front-matter" data-line={firstLine}>
              {split.block}
            </pre>
          )}
          {body.includes(GUIDE_PREFIX) ? (
            <GuideRendered body={body} lineBase={firstLine + split.lines - 1} baseLevel={baseLevel} dropTitle={dropTitle} firstParagraph={firstParagraph} cited={held} language={language} words={words} />
          ) : (
            <Rendered body={body} lineBase={firstLine + split.lines - 1} baseLevel={baseLevel} dropTitle={dropTitle} firstParagraph={firstParagraph} cited={held} guideTopics={NO_TOPICS} language={language} words={words} />
          )}
        </div>
      </AnchorContext>
    </LinkScopeContext>
  );
}
