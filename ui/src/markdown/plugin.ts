import type { ExtraProps, Options } from "react-markdown";
import {
  attributeBlock,
  definitionDestination,
  inlineDestination,
  isAnchorClose,
  isEmptyNamedAnchor,
  isLocalDestination,
  isNamedAnchorOpen,
  lineIndex,
  withoutComments,
} from "./source";

// The one remark plugin of the renderer (docs/features/ui-markdown.md "Description and interactions"): it
// rewrites the markdown tree before it becomes HTML elements, so nothing below needs to be trusted
// to the library's defaults.
// - Raw HTML is never interpreted: comments and empty named anchors (`<a id|name ...></a>`, `<a ... />`,
//   read as the core's `a_tag`) are dropped, the rest becomes literal text, inline or a paragraph of
//   its own, whitespace kept (handed to the output as is, never through the text handler's
//   trimming). No `raw` node reaches the output.
// - Headings shift below the pane's own (the shallowest lands at `baseLevel + 1`), capped at 6; a
//   trailing `{...}` attribute block is cut off, its `#ID` kept as a label.
// - Images become "Image: <alt>" and their path as text; footnotes a label and a block of their
//   own: no generated heading, `id` or in-page `href`.
// - Links carry what the anchor needs to match the daemon's links read: the destination exactly as
//   written and its line (the core's scan, `./source`), or the URL as text for an external one.
// - Every block carries `data-line`, its first line in the holder's file.
// Types come through react-markdown's own (no direct mdast or hast package).

type ToHastOptions = NonNullable<Options["remarkRehypeOptions"]>;
type Handler = NonNullable<NonNullable<ToHastOptions["handlers"]>["root"]>;
type ToHastState = Parameters<Handler>[0];

/** A node of the markdown tree (mdast), as react-markdown's types reach it. */
type MdNode = Parameters<ToHastState["all"]>[0];
type MdOf<T extends MdNode["type"]> = Extract<MdNode, { type: T }>;
type MdParent = Extract<MdNode, { children: unknown }>;
type MdRoot = MdOf<"root">;
type MdText = MdOf<"text">;
type MdParagraph = MdOf<"paragraph">;
type MdData = NonNullable<MdText["data"]>;
type MdPosition = NonNullable<MdNode["position"]>;
/** An HTML element (hast), as the components receive it. */
export type HastElement = NonNullable<ExtraProps["node"]>;
type HastContent = NonNullable<MdData["hChildren"]>[number];
type HastProperties = HastElement["properties"];

/** How an anchor is to be shown: matched against the links read, as text with its URL, or as text. */
export type LinkKind = "local" | "external" | "autolink" | "none";

export interface SpecOptions {
  /** The text parsed: offsets and lines below are into it. */
  source: string;
  /** Added to a line of `source` (1-based) to give its line in the file. */
  lineBase: number;
  /** The level of the heading the text sits under, 1-5. */
  baseLevel: number;
}

/** Block nodes that carry `data-line`. */
const BLOCKS = new Set<string>(["paragraph", "heading", "thematicBreak", "blockquote", "list", "listItem", "code", "table", "tableRow"]);

/** Parents whose children are blocks: an HTML node there is an HTML block. */
const BLOCK_PARENTS = new Set<string>(["root", "blockquote", "listItem", "footnoteDefinition"]);

/** The text without its trailing spaces and tabs, scanned from the end. */
function withoutTrailingBlanks(text: string): string {
  let end = text.length;
  while (end > 0 && (text[end - 1] === " " || text[end - 1] === "\t")) {
    end -= 1;
  }
  return text.slice(0, end);
}

function element(tagName: string, className: string, children: HastContent[]): HastContent {
  return { type: "element", tagName, properties: { className: [className] }, children };
}

function hastText(value: string): HastContent {
  return { type: "text", value };
}

/** Merges properties into what a node becomes. */
function setProperties(node: MdNode, properties: HastProperties): void {
  const data: MdData = node.data ?? {};
  data.hProperties = { ...data.hProperties, ...properties };
  node.data = data;
}

/** A text node shown as an element of the renderer's own: `tagName.className`, `children` inside. */
function shownAs(tagName: string, className: string, children: HastContent[], position: MdPosition | undefined): MdText {
  return {
    type: "text",
    value: "",
    position,
    data: { hName: tagName, hProperties: { className: [className] }, hChildren: children },
  };
}

function isParent(node: MdNode): node is MdParent {
  return "children" in node;
}

function walk(node: MdNode, visit: (node: MdNode) => void): void {
  visit(node);
  if (isParent(node)) {
    for (const child of node.children) {
      walk(child, visit);
    }
  }
}

class Rewrite {
  private readonly lineOf: (offset: number) => number;
  private readonly definitions = new Map<string, MdOf<"definition">>();
  private shift = 0;

  constructor(private readonly options: SpecOptions) {
    this.lineOf = lineIndex(options.source);
  }

  run(root: MdRoot): void {
    let shallowest = 7;
    walk(root, (node) => {
      if (node.type === "definition") {
        const key = node.identifier.toUpperCase();
        if (!this.definitions.has(key)) {
          this.definitions.set(key, node);
        }
      } else if (node.type === "heading") {
        shallowest = Math.min(shallowest, node.depth);
      }
    });
    this.shift = shallowest > 6 ? 0 : this.options.baseLevel + 1 - shallowest;
    this.children(root);
  }

  private fileLine(line: number): number {
    return this.options.lineBase + line;
  }

  /** Rewrites a parent's children in place, then descends. */
  private children(parent: MdParent): void {
    const block = BLOCK_PARENTS.has(parent.type);
    const kept: MdNode[] = [];
    const children: MdNode[] = parent.children;
    for (let index = 0; index < children.length; index += 1) {
      const child = children[index];
      if (child === undefined) {
        continue;
      }
      if (child.type === "html") {
        const next = children[index + 1];
        if (!block && isNamedAnchorOpen(child.value) && next?.type === "html" && isAnchorClose(next.value)) {
          index += 1;
          continue;
        }
        const shown = this.html(child, block);
        if (shown !== null) {
          kept.push(shown);
        }
        continue;
      }
      const replaced = this.node(child);
      if (isParent(replaced)) {
        this.children(replaced);
      }
      if (replaced.type === "heading") {
        // After its children: a comment dropped there may have parted the text from its `{...}`.
        this.attributes(replaced);
      }
      if (replaced.type === "paragraph" && replaced.children.every((part) => part.type === "text" && part.data?.hName === undefined && part.value.trim() === "")) {
        continue;
      }
      kept.push(replaced);
    }
    // The parent's own child type holds every node kept: each replacement is of the kind it replaced.
    (parent as { children: MdNode[] }).children = kept;
  }

  /**
   * Raw HTML as literal text, its whitespace kept: comments and an empty named anchor dropped; null
   * when nothing is left. The text goes out as given (`hChildren`), past the text handler's trimming.
   */
  private html(node: MdOf<"html">, block: boolean): MdNode | null {
    const value = withoutComments(node.value);
    if (value.trim() === "" || isEmptyNamedAnchor(value)) {
      return null;
    }
    if (!block) {
      return shownAs("span", "md-raw-inline", [hastText(value)], node.position);
    }
    const paragraph: MdParagraph = {
      type: "paragraph",
      children: [{ type: "text", value }],
      position: node.position,
      data: { hChildren: [hastText(value)] },
    };
    setProperties(paragraph, { className: ["md-raw-html"] });
    this.line(paragraph);
    return paragraph;
  }

  private line(node: MdNode): void {
    const line = node.position?.start.line;
    if (line !== undefined) {
      setProperties(node, { dataLine: this.fileLine(line) });
    }
  }

  /** One node rewritten (or replaced by what shows it); its children are rewritten after. */
  private node(node: MdNode): MdNode {
    if (BLOCKS.has(node.type)) {
      this.line(node);
    }
    switch (node.type) {
      case "heading":
        this.heading(node);
        return node;
      case "code":
        setProperties(node, { dataInfo: [node.lang, node.meta].filter((part) => part !== null && part !== undefined && part !== "").join(" ") });
        return node;
      case "link":
        this.link(node);
        return node;
      case "linkReference":
        this.linkReference(node);
        return node;
      case "image":
        return this.image(node.alt, node.url, node.position);
      case "imageReference": {
        const definition = this.definitions.get(node.identifier.toUpperCase());
        return this.image(node.alt, definition?.url ?? "", node.position);
      }
      case "footnoteReference":
        return shownAs("sup", "md-footnote-ref", [hastText(`[${node.label ?? node.identifier}]`)], node.position);
      case "footnoteDefinition":
        return this.footnote(node);
      default:
        return node;
    }
  }

  private heading(node: MdOf<"heading">): void {
    const depth = Math.min(6, Math.max(1, node.depth + this.shift));
    node.depth = depth as typeof node.depth;
  }

  /** A heading's trailing `{...}` cut off with the blanks before it; its `#ID` shown as a label after one space. */
  private attributes(node: MdOf<"heading">): void {
    const last = node.children[node.children.length - 1];
    if (last?.type !== "text" || last.data?.hName !== undefined) {
      return;
    }
    const block = attributeBlock(last.value);
    if (block === null) {
      return;
    }
    last.value = block.before;
    // The blanks the cut left at the end, across texts a dropped comment parted.
    for (let tail = node.children[node.children.length - 1]; tail?.type === "text" && tail.data?.hName === undefined; tail = node.children[node.children.length - 1]) {
      tail.value = withoutTrailingBlanks(tail.value);
      if (tail.value !== "") {
        break;
      }
      node.children.pop();
    }
    const id = block.inner
      .trim()
      .split(/\s+/)
      .find((token) => token.startsWith("#") && token.length > 1)
      ?.slice(1);
    if (id === undefined) {
      return;
    }
    if (node.children.length > 0) {
      node.children.push({ type: "text", value: " " });
    }
    node.children.push(shownAs("span", "md-heading-id", [hastText(id)], undefined));
  }

  private link(node: MdOf<"link">): void {
    const start = node.position?.start.offset;
    const end = node.position?.end.offset;
    if (start === undefined || end === undefined) {
      setProperties(node, { dataKind: "none" });
      return;
    }
    if (this.options.source[start] !== "[") {
      // `<https://...>` or a bare URL (GFM): its text is the URL.
      setProperties(node, { dataKind: "autolink" });
      return;
    }
    const lastPart = node.children[node.children.length - 1];
    const textEnd = lastPart?.position?.end.offset ?? start + 1;
    const place = inlineDestination(this.options.source, start, end, textEnd);
    this.destination(node, place === null ? null : this.options.source.slice(place.start, place.end), place?.start ?? null, node.url);
  }

  private linkReference(node: MdOf<"linkReference">): void {
    const definition = this.definitions.get(node.identifier.toUpperCase());
    const start = definition?.position?.start.offset;
    const end = definition?.position?.end.offset;
    if (definition === undefined || start === undefined || end === undefined) {
      setProperties(node, { dataKind: "none" });
      return;
    }
    const place = definitionDestination(this.options.source, start, end);
    this.destination(node, place === null ? null : this.options.source.slice(place.start, place.end), place?.start ?? null, definition.url);
  }

  private destination(node: MdNode, written: string | null, at: number | null, url: string): void {
    const shown = written ?? url;
    if (!isLocalDestination(shown)) {
      setProperties(node, { dataKind: shown.trim() === "" ? "none" : "external", dataUrl: shown });
      return;
    }
    if (written === null || at === null) {
      setProperties(node, { dataKind: "none" });
      return;
    }
    setProperties(node, { dataKind: "local", dataDest: written, dataDestLine: this.fileLine(this.lineOf(at)) });
  }

  private image(alt: string | null | undefined, url: string, position: MdPosition | undefined): MdText {
    const label = alt === null || alt === undefined || alt === "" ? "Image" : `Image: ${alt}`;
    return shownAs(
      "span",
      "md-image",
      [element("span", "md-image-alt", [hastText(label)]), hastText(" "), element("span", "md-image-path", [hastText(url)])],
      position,
    );
  }

  private footnote(node: MdOf<"footnoteDefinition">): MdNode {
    const label: MdParagraph = { type: "paragraph", children: [{ type: "text", value: `Footnote [${node.label ?? node.identifier}]` }] };
    setProperties(label, { className: ["md-footnote-label"] });
    const shown: MdOf<"blockquote"> = {
      type: "blockquote",
      children: [label, ...node.children],
      position: node.position,
      data: { hName: "div" },
    };
    setProperties(shown, { className: ["md-footnote"] });
    this.line(shown);
    return shown;
  }
}

/** The plugin: `[remarkSpec, options]` after remark-gfm. */
export function remarkSpec(options: SpecOptions) {
  return (tree: MdRoot): undefined => {
    new Rewrite(options).run(tree);
    return undefined;
  };
}

/** A property the plugin set, read back by a component. */
export function numberProperty(node: HastElement | undefined, key: string): number | null {
  const value = node?.properties[key];
  return typeof value === "number" ? value : null;
}

export function stringProperty(node: HastElement | undefined, key: string): string | null {
  const value = node?.properties[key];
  return typeof value === "string" ? value : null;
}
