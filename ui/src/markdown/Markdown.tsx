import { createContext, createElement, memo, use, useEffect, useMemo, useRef, type JSX, type MouseEvent } from "react";
import ReactMarkdown, { type Components, type ExtraProps } from "react-markdown";
import remarkGfm from "remark-gfm";
import type { ShownLink } from "../api/types";
import { sectionHash } from "../app/routes";
import { linkStateLook } from "../tree/labels";
import type { LineTarget } from "../tree/TextView";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { numberProperty, remarkSpec, stringProperty, type HastElement } from "./plugin";
import { splitFrontMatter } from "./source";

// The renderer (docs/features/ui-markdown.md; ADR-0036), loaded on demand like the graph canvas:
// the only module importing react-markdown and remark-gfm. React elements only, never an HTML
// string; raw HTML is text (./plugin); every component below writes the attributes it names and
// never spreads what it is given. An anchor exists only where the node's links read lists the
// link as resolved, its href from routes.ts.

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
  /** An anchor to another node was clicked. */
  onFollow?: (event: MouseEvent<HTMLAnchorElement>) => void;
  /** Classes of the text's container. */
  className?: string;
}

interface LinkScope {
  project: string;
  links: readonly ShownLink[] | null;
  path: string | null;
  onFollow: ((event: MouseEvent<HTMLAnchorElement>) => void) | undefined;
}

// The links read reaches the anchors through context: its landing re-renders them, never the parse.
const LinkScopeContext = createContext<LinkScope>({ project: "", links: null, path: null, onFollow: undefined });

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
  const entries = kind === "local" ? listed(scope, stringProperty(node, "dataDest"), numberProperty(node, "dataDestLine")) : [];
  const name = resolvedName(entries);
  if (name !== null) {
    return (
      <a className="md-link" href={sectionHash(scope.project, "tree", name)} onClick={scope.onFollow}>
        {children}
      </a>
    );
  }
  const unresolved = entries.find((entry) => entry.state !== "resolved");
  if (unresolved === undefined) {
    return <span className="md-link">{children}</span>;
  }
  return (
    <span className="md-link md-link-unresolved">
      {children} <Badge name="Link" look={linkStateLook(unresolved.state)} />
    </span>
  );
}

function heading(level: 1 | 2 | 3 | 4 | 5 | 6) {
  return function Heading({ node, children }: Props<"h1">) {
    return createElement(`h${String(level)}`, { className: "md-heading", "data-line": numberProperty(node, "dataLine") ?? undefined }, children);
  };
}

/** An image is never loaded: its alt text and path as text (./plugin turns each into that first). */
function Image({ alt }: Props<"img">) {
  return <span className="md-image">{alt === undefined || alt === "" ? "Image" : `Image: ${alt}`}</span>;
}

/** A task item's box: an icon and a word, never a form control. */
function TaskState({ checked }: Props<"input">) {
  const done = checked === true;
  return (
    <span className="md-task-state">
      <Icon name={done ? "taskItemDone" : "taskItemOpen"} />
      <span>{done ? "Done" : "Not done"}</span>
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
  const line = numberProperty(node, "dataLine");
  return (
    <div className="md-table-region" role="region" aria-label={line === null ? "Table" : `Table at line ${String(line)}`} tabIndex={0} data-line={line ?? undefined}>
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

/** The parse and its elements: again only when the text, its lines or its level change. */
const Rendered = memo(function Rendered({ body, lineBase, baseLevel }: { body: string; lineBase: number; baseLevel: number }) {
  return (
    <ReactMarkdown remarkPlugins={[remarkGfm, [remarkSpec, { source: body, lineBase, baseLevel }]]} components={COMPONENTS}>
      {body}
    </ReactMarkdown>
  );
});

/** Markdown prose as React elements; a document's front-matter verbatim above it. */
export function Markdown({ source, links, project, baseLevel, frontMatter, firstLine = 1, path = null, target = null, onFollow, className }: MarkdownProps) {
  const container = useRef<HTMLDivElement>(null);
  const split = useMemo(() => (frontMatter ? splitFrontMatter(source) : { block: null, lines: 0, body: source }), [source, frontMatter]);
  // A leading BOM with no front-matter: the parser skips it without counting it in its offsets, so
  // it is dropped here (a line of its own it is not) and every offset stays the text's.
  const body = split.block === null && split.body.startsWith("\uFEFF") ? split.body.slice(1) : split.body;
  const scope = useMemo<LinkScope>(() => ({ project, links, path, onFollow }), [project, links, path, onFollow]);

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
      <div ref={container} className={className === undefined ? "markdown" : `markdown ${className}`}>
        {split.block !== null && (
          <pre className="md-front-matter" data-line={firstLine}>
            {split.block}
          </pre>
        )}
        <Rendered body={body} lineBase={firstLine + split.lines - 1} baseLevel={baseLevel} />
      </div>
    </LinkScopeContext>
  );
}
