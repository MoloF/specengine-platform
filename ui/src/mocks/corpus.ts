import { ClientError, type BundleOptions, type GraphOptions, type NodeOptions, type SearchOptions, type TreeOptions } from "../api/client";
import type {
  BundleItem,
  BundleLayers,
  BundleVia,
  BundleView,
  Direction,
  FollowedType,
  GraphEdge,
  GraphNode,
  GraphView,
  LeftOut,
  NodeView,
  SearchHit,
  SearchResults,
  ShownLink,
  ShownLinks,
  ShownNode,
  Snippet,
  SnippetSegment,
  TailEntry,
  TreeNode,
  TreeView,
  WorkingAnswer,
} from "../api/types";
import { fakeHex, tokensEst, type MockCorpus, type MockDocument, type MockLink, type MockNode } from "./build";

// The reads `spec serve` answers for a browser (daemon-read): tree, nodes and search uncut,
// search snippets as structure, refusals as the CLI words them (exit 2, here 503).

/** Exit 2 as the daemon answers it: 503 with the CLI's line. */
function refusal(message: string): ClientError {
  return new ClientError({ status: 503, message: `spec: ${message}` });
}

/** What a tree, a link or a bundle lists a node by: its ID, else its path. */
export function nameOf(node: MockNode): string {
  return node.id ?? node.path;
}

function byPlace(a: MockNode, b: MockNode): number {
  return a.path < b.path ? -1 : a.path > b.path ? 1 : a.line - b.line;
}

/** Lookups over a corpus, built once per corpus (the `large` scenario has 3 000+ nodes). */
interface CorpusIndex {
  /** Every node by (path, line). */
  nodes: MockNode[];
  files: Map<string, MockDocument>;
  /** A name's first node in (path, line): where a link or a `parent:` lands. */
  firstByName: Map<string, MockNode>;
  /** Documents by the name of their tree parent, by (path, line). */
  childDocuments: Map<string, MockNode[]>;
}

const indexes = new WeakMap<MockCorpus, CorpusIndex>();

function indexOf(corpus: MockCorpus): CorpusIndex {
  const known = indexes.get(corpus);
  if (known !== undefined) {
    return known;
  }
  const nodes = corpus.documents.flatMap((file) => [file.node, ...file.sections]).sort(byPlace);
  const firstByName = new Map<string, MockNode>();
  for (const node of nodes) {
    if (!firstByName.has(nameOf(node))) {
      firstByName.set(nameOf(node), node);
    }
  }
  const childDocuments = new Map<string, MockNode[]>();
  for (const node of nodes) {
    if (node.isDocument && node.parent !== null) {
      childDocuments.set(node.parent, [...(childDocuments.get(node.parent) ?? []), node]);
    }
  }
  const built: CorpusIndex = {
    nodes,
    files: new Map(corpus.documents.map((file) => [file.path, file])),
    firstByName,
    childDocuments,
  };
  indexes.set(corpus, built);
  return built;
}

function allNodes(corpus: MockCorpus): MockNode[] {
  return indexOf(corpus).nodes;
}

function fileOf(corpus: MockCorpus, node: MockNode): MockDocument {
  const file = indexOf(corpus).files.get(node.path);
  if (file === undefined) {
    throw new Error(`no mock file ${node.path}`);
  }
  return file;
}

/** The file name without `.md`: the `slug` of a `slug/ID` REF. */
function stem(path: string): string {
  return (path.split("/").pop() ?? path).replace(/\.md$/, "");
}

/** Whether `inner` lies in `outer`'s span (the same file, inside its lines), itself excluded. */
function within(inner: MockNode, outer: MockNode): boolean {
  return inner !== outer && inner.path === outer.path && inner.line >= outer.line && inner.endLine <= outer.endLine;
}

/** The innermost node of a file holding a line: an ID section, else the document. */
function holderOfLine(corpus: MockCorpus, path: string, line: number): MockNode | null {
  const file = indexOf(corpus).files.get(path);
  if (file === undefined) {
    return null;
  }
  const around = file.sections.filter((section) => section.line <= line && line <= section.endLine);
  return around.sort((a, b) => b.line - a.line)[0] ?? file.node;
}

/** The node a name lands on: the first in (path, line) when several hold it. */
function nodeNamed(corpus: MockCorpus, name: string): MockNode | null {
  return indexOf(corpus).firstByName.get(name) ?? null;
}

// Look-alike letters a REF may carry, with their Latin forms (the five the spec names).
const LOOK_ALIKES: Record<string, string> = {
  "\u0410": "A",
  "\u0415": "E",
  "\u041e": "O",
  "\u0420": "P",
  "\u0421": "C",
};

type Resolved = { holders: MockNode[] } | { reason: string };

/**
 * What a REF names: an ID, `slug/ID`, `ID#SECTION` or a root-relative `.md` path. A REF with
 * letters outside ASCII is refused with its Latin form, as `spec show` does (exit 2).
 */
export function resolveRef(corpus: MockCorpus, ref: string): Resolved {
  const written = ref.trim();
  if (written.endsWith(".md")) {
    const file = corpus.documents.find((candidate) => candidate.path === written);
    return file === undefined
      ? { reason: `\`${written}\` is no indexed document: not under the \`[paths]\` roots, excluded, or missing` }
      : { holders: [file.node] };
  }
  if (/[^\x20-\x7e]/.test(written)) {
    const fix = Array.from(written, (char) => LOOK_ALIKES[char] ?? char).join("");
    throw refusal(
      /^[\x20-\x7e]*$/.test(fix)
        ? `\`${written}\` mixes scripts or uses look-alike letters; IDs are Latin only: write \`${fix}\``
        : `\`${written}\` mixes scripts; IDs are Latin only: write it in Latin letters`,
    );
  }
  const hash = written.indexOf("#");
  const base = hash < 0 ? written : written.slice(0, hash);
  const section = hash < 0 ? null : written.slice(hash + 1);
  const slash = base.lastIndexOf("/");
  const slug = slash < 0 ? null : base.slice(0, slash);
  const id = slash < 0 ? base : base.slice(slash + 1);
  let holders = allNodes(corpus).filter((node) => node.id === id && (slug === null || stem(node.path) === slug));
  if (section !== null) {
    const nodes = allNodes(corpus);
    holders = holders.flatMap((holder) => nodes.filter((node) => node.id === section && within(node, holder)));
  }
  return holders.length === 0 ? { reason: `\`${written}\` resolves to no ID and no alias` } : { holders };
}

// spec tree

function childrenOf(corpus: MockCorpus, node: MockNode): MockNode[] {
  const file = fileOf(corpus, node);
  const sections = file.sections.filter((section) => (node.isDocument ? section.within === null : section.within === node.id));
  const name = nameOf(node);
  const documents = nodeNamed(corpus, name) === node ? (indexOf(corpus).childDocuments.get(name) ?? []) : [];
  return [...sections, ...documents];
}

function treeRow(corpus: MockCorpus, node: MockNode, depth: number, parent: string | null): TreeNode {
  return {
    id: node.id,
    kind: node.kind,
    title: node.title,
    path: node.path,
    line: node.line,
    depth,
    parent,
    mark: node.isDocument ? node.mark : null,
    status: node.status,
    rev: node.rev,
    tokens_est: tokensEst(spanText(node, fileOf(corpus, node))),
    archived: node.archived,
  };
}

/** `spec tree --json` over the corpus: pre-order, sections before child documents, uncut. */
export function treeOf(corpus: MockCorpus, options: TreeOptions = {}): TreeView {
  const archive = options.archive ?? false;
  const kinds = options.kinds ?? [];
  const leftOut: LeftOut = { generated: 0, tier3: 0 };
  const base = {
    ref: options.root ?? null,
    depth: options.depth ?? null,
    kinds,
    archive,
    truncated: false,
  };
  let starts: MockNode[];
  if (options.root !== undefined) {
    const resolved = resolveRef(corpus, options.root);
    if ("reason" in resolved) {
      return { ...base, reason: resolved.reason, notes: [], left_out: leftOut, nodes: [] };
    }
    starts = resolved.holders;
  } else {
    starts = corpus.documents
      .map((file) => file.node)
      .filter((node) => node.parent === null)
      .sort(byPlace);
  }
  const rows: TreeNode[] = [];
  const visit = (node: MockNode, depth: number, parent: string | null, admitted: boolean) => {
    if (!admitted && node.isDocument) {
      if (node.generated) {
        leftOut.generated += 1;
        return;
      }
      if (node.archived && !archive) {
        leftOut.tier3 += 1;
        return;
      }
    }
    if (options.depth !== undefined && depth > options.depth) {
      return;
    }
    rows.push(treeRow(corpus, node, depth, parent));
    for (const child of childrenOf(corpus, node)) {
      visit(child, depth + 1, nameOf(node), false);
    }
  };
  for (const start of starts) {
    visit(start, 0, null, options.root !== undefined);
  }
  return {
    ...base,
    reason: null,
    notes: options.root === undefined ? [...corpus.treeNotes] : [],
    left_out: leftOut,
    nodes: kinds.length === 0 ? rows : rows.filter((row) => row.kind !== null && kinds.includes(row.kind)),
  };
}

// spec show [--links]

/** A node's bytes as `spec show` prints them: a document's file, a section's span, `\n`-ended. */
function spanText(node: MockNode, file: MockDocument): string {
  return `${file.lines.slice(node.line - 1, node.endLine).join("\n")}\n`;
}

function shownNode(corpus: MockCorpus, node: MockNode, links: ShownLinks | null): ShownNode {
  const file = fileOf(corpus, node);
  const text = spanText(node, file);
  return {
    id: node.id,
    kind: node.kind,
    title: node.title,
    path: node.path,
    line: node.line,
    end_line: node.endLine,
    status: node.status,
    rev: node.rev,
    tokens_est: tokensEst(text),
    archived: node.archived,
    utf8: node.utf8,
    sections: file.sections.filter((section) => section.id !== null && within(section, node)).map((section) => section.id ?? ""),
    span_hash: `b3:${fakeHex(`${node.path}\n${text}`, 64)}`,
    text,
    truncated: false,
    omitted: null,
    links,
  };
}

/** Strong before `mentions`, then (type, path, line), as `spec show --links` lists them. */
function linkOrder(a: ShownLink, b: ShownLink): number {
  const weak = Number(a.type === "mentions") - Number(b.type === "mentions");
  if (weak !== 0) {
    return weak;
  }
  if (a.type !== b.type) {
    return a.type < b.type ? -1 : 1;
  }
  if (a.path !== b.path) {
    return a.path < b.path ? -1 : 1;
  }
  return a.line - b.line;
}

/** Whether a link lands on the node or on a section within it. */
function landsOn(corpus: MockCorpus, written: MockLink, node: MockNode): MockNode | null {
  if (written.state !== "resolved" || written.to === null) {
    return null;
  }
  const target = nodeNamed(corpus, written.to);
  if (target === null) {
    return null;
  }
  return target === node || within(target, node) ? target : null;
}

/** A node's links: those written in its span, those landing on it or a section within it. */
export function linksOf(corpus: MockCorpus, node: MockNode, archive: boolean): ShownLinks {
  const leftOut: LeftOut = { generated: 0, tier3: 0 };
  const outgoing: ShownLink[] = [];
  const incoming: ShownLink[] = [];
  for (const written of corpus.links) {
    const source = holderOfLine(corpus, written.path, written.line);
    if (source === null) {
      continue;
    }
    const inSpan = written.path === node.path && node.line <= written.line && written.line <= node.endLine;
    if (inSpan) {
      outgoing.push({
        type: written.type,
        origin: written.origin,
        at: source === node ? null : source.id,
        name: written.state === "resolved" ? written.to : null,
        written: written.written,
        path: written.path,
        line: written.line,
        state: written.state,
        reason: written.reason,
      });
      continue;
    }
    const landed = landsOn(corpus, written, node);
    if (landed === null) {
      continue;
    }
    const sourceFile = fileOf(corpus, source).node;
    if (sourceFile.path !== node.path) {
      if (sourceFile.generated) {
        leftOut.generated += 1;
        continue;
      }
      if (sourceFile.archived && !archive) {
        leftOut.tier3 += 1;
        continue;
      }
    }
    incoming.push({
      type: written.type,
      origin: written.origin,
      at: landed === node ? null : landed.id,
      name: nameOf(source),
      written: written.written,
      path: written.path,
      line: written.line,
      state: written.state,
      reason: written.reason,
    });
  }
  return { outgoing: outgoing.sort(linkOrder), incoming: incoming.sort(linkOrder), left_out: leftOut, omitted: 0 };
}

/** `spec show --json`: the holders of a REF, or the reason there are none (exit 1). */
export function nodeViewOf(corpus: MockCorpus, ref: string, options: NodeOptions = {}): NodeView {
  const links = (options.with ?? []).includes("links");
  if (options.archive === true && !links) {
    throw refusal("--archive applies to --links only: add --links, or drop --archive");
  }
  const resolved = resolveRef(corpus, ref);
  if ("reason" in resolved) {
    return { ref, reason: resolved.reason, notes: [], nodes: [] };
  }
  return {
    ref,
    reason: null,
    notes: [],
    nodes: resolved.holders.map((holder) =>
      shownNode(corpus, holder, links ? linksOf(corpus, holder, options.archive ?? false) : null),
    ),
  };
}

// spec search

const MIN_TERM_CHARS = 3;
const DEFAULT_LIMIT = 20;
const SNIPPET_CONTEXT = 32;

/** Lower case one code unit at a time, so every index of the folded text is the original's. */
function fold(text: string): string {
  let out = "";
  for (const unit of text.split("")) {
    const lower = unit.toLowerCase();
    out += lower.length === 1 ? lower : unit;
  }
  return out;
}

/** Every [start, end) where a term occurs, merged where they touch. */
function occurrences(text: string, terms: string[]): [number, number][] {
  const folded = fold(text);
  const found: [number, number][] = [];
  for (const term of terms) {
    for (let at = folded.indexOf(term); at >= 0; at = folded.indexOf(term, at + 1)) {
      found.push([at, at + term.length]);
    }
  }
  found.sort((a, b) => a[0] - b[0]);
  const merged: [number, number][] = [];
  for (const range of found) {
    const last = merged[merged.length - 1];
    if (last !== undefined && range[0] <= last[1]) {
      last[1] = Math.max(last[1], range[1]);
    } else {
      merged.push([range[0], range[1]]);
    }
  }
  return merged;
}

/** 32 characters around the first hit, every hit in it marked, the cut ends flagged. */
function snippetOf(text: string, terms: string[]): Snippet | null {
  const ranges = occurrences(text, terms);
  const first = ranges[0];
  if (first === undefined) {
    return null;
  }
  const start = Math.max(0, first[0] - SNIPPET_CONTEXT);
  const end = Math.min(text.length, first[1] + SNIPPET_CONTEXT);
  const segments: SnippetSegment[] = [];
  let at = start;
  for (const [from, to] of ranges) {
    if (from >= end) {
      break;
    }
    const hitStart = Math.max(from, start);
    const hitEnd = Math.min(to, end);
    if (hitStart > at) {
      segments.push({ text: text.slice(at, hitStart), hit: false });
    }
    segments.push({ text: text.slice(hitStart, hitEnd), hit: true });
    at = hitEnd;
  }
  if (at < end) {
    segments.push({ text: text.slice(at, end), hit: false });
  }
  return { segments, cut_start: start > 0, cut_end: end < text.length };
}

/**
 * `spec search --json` as the store answers: terms of three or more characters, ANDed, each a
 * case-folded substring of the title or the text (no Unicode normalisation); hits by (path, line);
 * Tier 3 dropped before the limit unless `archive`, and counted.
 */
export function searchOf(corpus: MockCorpus, options: SearchOptions): SearchResults {
  const limit = options.limit ?? DEFAULT_LIMIT;
  if (!Number.isInteger(limit) || limit < 1 || limit > 200) {
    throw refusal(`--limit ${String(limit)}: the limit is 1 to 200`);
  }
  const words = options.query.split(/\s+/).filter((word) => word !== "");
  const kept = words.filter((word) => Array.from(word).length >= MIN_TERM_CHARS);
  const dropped = words.filter((word) => Array.from(word).length < MIN_TERM_CHARS);
  if (kept.length === 0) {
    throw refusal(
      `no search term of ${String(MIN_TERM_CHARS)} or more characters; to read a node by its ID, use \`spec show <ID>\``,
    );
  }
  const notes =
    dropped.length === 0
      ? []
      : [`search terms shorter than ${String(MIN_TERM_CHARS)} characters dropped: ${dropped.map((word) => `\`${word}\``).join(", ")}`];
  const terms = kept.map(fold);
  const archive = options.archive ?? false;
  const kinds = options.kinds ?? [];
  let tier3 = 0;
  const hits: SearchHit[] = [];
  for (const node of allNodes(corpus)) {
    if (kinds.length > 0 && (node.kind === null || !kinds.includes(node.kind))) {
      continue;
    }
    const text = spanText(node, fileOf(corpus, node));
    const title = node.title ?? "";
    const haystack = fold(`${title}\n${text}`);
    if (!terms.every((term) => haystack.includes(term))) {
      continue;
    }
    if (node.archived && !archive) {
      tier3 += 1;
      continue;
    }
    hits.push({
      id: node.id,
      kind: node.kind,
      title: node.title,
      path: node.path,
      line: node.line,
      ord: fileOf(corpus, node).sections.indexOf(node) + 1,
      archived: node.archived,
      snippet: snippetOf(text, terms) ?? snippetOf(title, terms),
    });
  }
  return {
    archive,
    hits: hits.slice(0, limit),
    kinds,
    limit,
    notes,
    query: options.query,
    tier3_left_out: tier3,
    truncated: false,
  };
}

// spec bundle

const DEFAULT_BUNDLE_BUDGET = 2000;
const BUNDLE_TAIL_LINES = 20;

/** The layers in print order with the CLI's headings. */
const LAYERS: readonly [keyof BundleLayers, string][] = [
  ["targets", "Targets"],
  ["open_questions", "Open questions"],
  ["ancestors", "Ancestors"],
  ["criteria", "Criteria"],
  ["bindings", "Bindings"],
  ["decisions", "Decisions"],
  ["neighbours", "Neighbours"],
  ["terms", "Terms"],
  ["tests", "Tests"],
];

/** Which links bring a candidate into which layer, in the target's direction. */
const BY_LINK: readonly [keyof BundleLayers, string, "in" | "out" | "both"][] = [
  ["criteria", "verifies", "in"],
  ["decisions", "canon", "in"],
  ["neighbours", "depends_on", "out"],
  ["neighbours", "constrains", "both"],
  ["neighbours", "derived_from", "out"],
  ["terms", "uses_term", "out"],
];

interface Candidate {
  node: MockNode;
  layer: keyof BundleLayers;
  via: BundleVia[];
}

function header(node: MockNode): string {
  return `${nameOf(node)} | ${node.kind ?? "-"} | ${node.title ?? "-"} | ${node.path}:${String(node.line)}`;
}

function emptyBundle(refs: string[], reason: string): BundleView {
  return {
    refs,
    reason,
    notes: [],
    task: null,
    budget: null,
    tokens: null,
    chars: null,
    bytes: null,
    bundle_hash: null,
    body: null,
    layers: null,
    tail: null,
    more: null,
  };
}

function isLive(corpus: MockCorpus, node: MockNode): boolean {
  const file = fileOf(corpus, node).node;
  return !file.archived && !file.generated;
}

/** The parent chain of a node in the tree, nearest first (ancestors layer). */
function ancestorsOf(corpus: MockCorpus, node: MockNode): MockNode[] {
  const chain: MockNode[] = [];
  let current: MockNode = node;
  for (;;) {
    let next: MockNode | null;
    if (!current.isDocument) {
      next = current.within === null ? fileOf(corpus, current).node : nodeNamed(corpus, current.within);
    } else {
      next = current.parent === null ? null : nodeNamed(corpus, current.parent);
    }
    if (next === null || next === node || chain.includes(next)) {
      return chain;
    }
    chain.push(next);
    current = next;
  }
}

/**
 * `spec bundle --json` over the corpus: the targets in their fullest form that fits, then the
 * linked layers greedily, the rest named in the tail. Under the frame's minimum: refused (exit 2).
 */
export function bundleOf(corpus: MockCorpus, options: BundleOptions): BundleView {
  const refs = options.node_ids;
  const targets: MockNode[] = [];
  for (const ref of refs) {
    const resolved = resolveRef(corpus, ref);
    if ("reason" in resolved) {
      return emptyBundle(refs, resolved.reason);
    }
    for (const holder of resolved.holders) {
      if (!targets.includes(holder)) {
        targets.push(holder);
      }
    }
  }
  targets.sort(byPlace);
  const budget = options.budget ?? DEFAULT_BUNDLE_BUDGET;
  const source = options.budget === undefined ? `the default budget of ${String(budget)}` : `--budget ${String(budget)}`;
  const covered = (node: MockNode) => targets.some((target) => target === node || within(node, target));

  // Candidates: each node once, in its first layer.
  const candidates: Candidate[] = [];
  const add = (node: MockNode, layer: keyof BundleLayers, via: BundleVia | null) => {
    if (covered(node) || !isLive(corpus, node)) {
      return;
    }
    const known = candidates.find((candidate) => candidate.node === node);
    if (known !== undefined) {
      if (known.layer === layer && via !== null && !known.via.some((v) => v.type === via.type && v.direction === via.direction)) {
        known.via.push(via);
      }
      return;
    }
    candidates.push({ node, layer, via: via === null ? [] : [via] });
  };
  interface Touching {
    written: MockLink;
    other: MockNode;
    direction: "in" | "out";
  }
  const touching = corpus.links.flatMap((written): Touching[] => {
    const source = holderOfLine(corpus, written.path, written.line);
    const target = written.state === "resolved" && written.to !== null ? nodeNamed(corpus, written.to) : null;
    if (source === null) {
      return [];
    }
    if (covered(source) && target !== null && !covered(target)) {
      return [{ written, other: target, direction: "out" }];
    }
    if (target !== null && covered(target) && !covered(source)) {
      return [{ written, other: source, direction: "in" }];
    }
    return [];
  });
  // Open questions: linked either way, holding a working answer.
  for (const { written, other, direction } of touching) {
    const holdsAnswer = corpus.links.some(
      (candidate) => candidate.type === "working_answer" && holderOfLine(corpus, candidate.path, candidate.line) === other,
    );
    if (holdsAnswer) {
      add(other, "open_questions", { type: written.type, direction });
    }
  }
  for (const target of targets) {
    for (const ancestor of ancestorsOf(corpus, target)) {
      add(ancestor, "ancestors", null);
    }
  }
  for (const [layer, type, way] of BY_LINK) {
    for (const { written, other, direction } of touching) {
      if (written.type === type && (way === "both" || way === direction)) {
        add(other, layer, { type, direction });
      }
    }
  }
  const order = (layer: keyof BundleLayers) => LAYERS.findIndex(([key]) => key === layer);
  candidates.sort((a, b) => order(a.layer) - order(b.layer) || (a.layer === "ancestors" ? 0 : byPlace(a.node, b.node)));

  // The frame and its minimum.
  const reserveCount = candidates.length;
  const reserve = reserveCount === 0 ? "" : `\n## Not included\n- ${String(reserveCount)} more\n`;
  const marked = targets.map((target) => `${header(target)} | outline\n`).join("");
  const title = `# Bundle: ${targets.map(nameOf).join(", ")}\n\n## Targets\n`;
  const minimum = tokensEst(`${title}${marked}${reserve}`);
  if (budget < minimum) {
    throw refusal(
      `${source} is below this bundle's minimum of ${String(minimum)} tokens (its title, target headers and not-included line): raise the budget to ${String(minimum)} or more`,
    );
  }
  const fits = (body: string) => tokensEst(`${body}${reserve}`) <= budget && Array.from(body).length <= 40000;

  const layers: BundleLayers = {
    targets: [],
    open_questions: [],
    ancestors: [],
    criteria: [],
    bindings: [],
    decisions: [],
    neighbours: [],
    terms: [],
    tests: [],
  };
  let body = title;
  targets.forEach((target, index) => {
    const file = fileOf(corpus, target);
    const text = spanText(target, file);
    const shown = `${header(target)} | ${String(tokensEst(text))} tokens${target.status === null ? "" : ` | status ${target.status}`}\n${text}`;
    const outline = target.summary === null ? null : `${header(target)} | outline\n${target.summary}\n`;
    const gap = index === 0 ? "" : "\n";
    let form: BundleItem["form"] = "header";
    if (fits(`${body}${gap}${shown}`)) {
      body += `${gap}${shown}`;
      form = "text";
    } else if (outline !== null && fits(`${body}${gap}${outline}`)) {
      body += `${gap}${outline}`;
      form = "outline";
    } else {
      body += `${gap}${header(target)} | outline\n`;
    }
    layers.targets.push(item(corpus, target, form, null, null));
  });
  const left: Candidate[] = [];
  for (const candidate of candidates) {
    const { node, layer } = candidate;
    const heading = LAYERS.find(([key]) => key === layer)?.[1] ?? layer;
    const opens = layers[layer].length === 0 ? `\n## ${heading}\n` : "\n";
    const via = candidate.via.length === 0 ? "" : ` | via ${candidate.via.map((v) => `${v.type} ${v.direction}`).join(", ")}`;
    const status = layer === "open_questions" && node.status !== null ? ` | status ${node.status}` : "";
    const answer = layer === "open_questions" ? workingAnswerOf(corpus, node) : null;
    let text = `${header(node)}${status}${via}\n`;
    let form: BundleItem["form"] = "header";
    if (layer === "criteria") {
      text += spanText(node, fileOf(corpus, node));
      form = "text";
    } else if (node.summary !== null) {
      text += `${node.summary}\n`;
      form = "summary";
    }
    if (answer !== null) {
      text += `working answer: ${answer.name ?? answer.written} | ${answer.path}:${String(answer.line)}\n`;
    }
    if (fits(`${body}${opens}${text}`)) {
      body += `${opens}${text}`;
      layers[layer].push(item(corpus, node, form, layer === "ancestors" ? null : candidate.via, answer));
    } else {
      left.push(candidate);
    }
  }
  const tail: TailEntry[] = [];
  let tailText = left.length === 0 ? "" : "\n## Not included\n";
  for (const candidate of left) {
    if (tail.length >= BUNDLE_TAIL_LINES) {
      break;
    }
    const node = candidate.node;
    const tokens = tokensEst(spanText(node, fileOf(corpus, node)));
    const line = `- ${nameOf(node)} | ${node.title ?? "-"} | ${String(tokens)} tokens\n`;
    if (!fits(`${body}${tailText}${line}`)) {
      break;
    }
    tailText += line;
    tail.push({ name: nameOf(node), title: node.title, path: node.path, line: node.line, tokens_est: tokens, layer: candidate.layer });
  }
  const more = left.length - tail.length;
  if (more > 0) {
    tailText += `- ${String(more)} more\n`;
  }
  body += tailText;
  return {
    refs,
    reason: null,
    notes: [],
    task: null,
    budget,
    tokens: tokensEst(body),
    chars: Array.from(body).length,
    bytes: new TextEncoder().encode(body).length,
    bundle_hash: `b3:${fakeHex(body, 64)}`,
    body,
    layers,
    tail,
    more,
  };
}

function item(
  corpus: MockCorpus,
  node: MockNode,
  form: BundleItem["form"],
  via: BundleVia[] | null,
  answer: WorkingAnswer | null,
): BundleItem {
  return {
    name: nameOf(node),
    kind: node.kind,
    title: node.title,
    path: node.path,
    line: node.line,
    form,
    status: node.isDocument ? node.status : null,
    via,
    working_answer: answer,
    tokens_est: tokensEst(spanText(node, fileOf(corpus, node))),
    archived: node.archived,
  };
}

function workingAnswerOf(corpus: MockCorpus, node: MockNode): WorkingAnswer | null {
  const written = corpus.links.find(
    (candidate) => candidate.type === "working_answer" && holderOfLine(corpus, candidate.path, candidate.line) === node,
  );
  if (written === undefined) {
    return null;
  }
  return {
    name: written.state === "resolved" ? written.to : null,
    written: written.written,
    path: written.path,
    line: written.line,
    state: written.state,
  };
}

// spec graph

/** The shared link types, in `specengine-model`'s `LINK_TYPES` order (`crates/specengine-model/src/link.rs`). */
const LINK_TYPES = [
  "derived_from",
  "depends_on",
  "constrains",
  "supersedes",
  "revises",
  "amends",
  "answers",
  "working_answer",
  "uses_term",
  "canon",
  "verifies",
  "adopts",
] as const;

/** `IMPACT_LINK_TYPES`, in the model's order: what an edit of a node reaches, and which way. */
const IMPACT_LINK_TYPES: readonly (readonly [string, Direction])[] = [
  ["depends_on", "in"],
  ["derived_from", "in"],
  ["verifies", "in"],
  ["uses_term", "in"],
  ["constrains", "out"],
];

const WEAK_LINK = "mentions";

function impactDirection(type: string): Direction | null {
  return IMPACT_LINK_TYPES.find(([name]) => name === type)?.[1] ?? null;
}

/** `graph.rs` `follow`: the direction a type is followed in, if at all. */
function followOf(options: GraphOptions): (type: string) => Direction | null {
  const given = options.types ?? [];
  const impact = options.impact === true;
  return (type) => {
    if (given.length === 0) {
      if (impact) {
        return impactDirection(type);
      }
      return type === WEAK_LINK ? null : "out";
    }
    if (!given.includes(type)) {
      return null;
    }
    return impact ? (impactDirection(type) ?? "in") : "out";
  };
}

/**
 * `graph.rs` `followed`: the `--type`s in the order given (a repeat once); else the impact table;
 * else the twelve shared types, then the corpus's unknown declared types by name.
 */
function followedOf(corpus: MockCorpus, options: GraphOptions): FollowedType[] {
  const follow = followOf(options);
  const given = options.types ?? [];
  if (given.length > 0) {
    const seen = new Set<string>();
    return given.flatMap((type) => {
      const direction = follow(type);
      if (seen.has(type) || direction === null) {
        return [];
      }
      seen.add(type);
      return [{ type, direction }];
    });
  }
  if (options.impact === true) {
    return IMPACT_LINK_TYPES.map(([type, direction]) => ({ type, direction }));
  }
  const shared: readonly string[] = LINK_TYPES;
  const unknown = [...new Set(corpus.links.map((written) => written.type))]
    .filter((type) => !shared.includes(type) && type !== WEAK_LINK)
    .sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
  return [...shared, ...unknown].map((type) => ({ type, direction: "out" }));
}

/** A link with its ends as the spec graph resolves them: the holder of its line, the nodes it lands on. */
interface WalkLink {
  written: MockLink;
  source: MockNode;
  targets: MockNode[];
}

interface GraphIndex {
  /** Links by the node holding their line. */
  from: Map<MockNode, WalkLink[]>;
  /** Resolved links by each node they land on. */
  into: Map<MockNode, WalkLink[]>;
}

const graphIndexes = new WeakMap<MockCorpus, GraphIndex>();

function graphIndexOf(corpus: MockCorpus): GraphIndex {
  const known = graphIndexes.get(corpus);
  if (known !== undefined) {
    return known;
  }
  const byName = new Map<string, MockNode[]>();
  for (const node of allNodes(corpus)) {
    byName.set(nameOf(node), [...(byName.get(nameOf(node)) ?? []), node]);
  }
  const from = new Map<MockNode, WalkLink[]>();
  const into = new Map<MockNode, WalkLink[]>();
  for (const written of corpus.links) {
    const source = holderOfLine(corpus, written.path, written.line);
    if (source === null) {
      continue;
    }
    const targets = written.state === "resolved" && written.to !== null ? (byName.get(written.to) ?? []) : [];
    const resolved: WalkLink = { written, source, targets };
    from.set(source, [...(from.get(source) ?? []), resolved]);
    for (const target of targets) {
      into.set(target, [...(into.get(target) ?? []), resolved]);
    }
  }
  const built: GraphIndex = { from, into };
  graphIndexes.set(corpus, built);
  return built;
}

/** The node and the ID sections nested in it (`spec_graph.rs` `within`). */
function nested(corpus: MockCorpus, node: MockNode): MockNode[] {
  return [node, ...fileOf(corpus, node).sections.filter((section) => within(section, node))];
}

function compareText(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

/**
 * `spec graph --json` as the browser reads it, uncut (`crates/specengine-cli/src/graph.rs`, the walk of
 * `crates/specengine-core/src/check/spec_graph.rs`): breadth-first from the REF's holders (distance 0),
 * each node visited once, a visited node's edge still listed; a node's links include its nested
 * sections'; a node at distance `depth` is not expanded. Edges written in a left-out file (generated;
 * archived without `archive`; never the REF's own files) are counted, not followed. Nodes by
 * (distance, path, line); edges by (type, path, line, written), in link direction.
 */
export function graphOf(corpus: MockCorpus, options: GraphOptions): GraphView {
  if (options.depth !== undefined && (!Number.isInteger(options.depth) || options.depth < 0)) {
    throw refusal(`--depth ${String(options.depth)}: the depth is an integer of 0 or more`);
  }
  const answer = {
    ref: options.ref,
    impact: options.impact ?? false,
    depth: options.depth ?? null,
    archive: options.archive ?? false,
    notes: [],
    truncated: false,
  };
  const resolved = resolveRef(corpus, options.ref);
  if ("reason" in resolved) {
    return { ...answer, reason: resolved.reason, types: [], left_out: { generated: 0, tier3: 0 }, nodes: [], edges: [] };
  }
  const types = followedOf(corpus, options);
  const follow = followOf(options);
  const asked = new Set(resolved.holders.map((holder) => holder.path));
  const leftOutEdges = new Map<MockLink, MockDocument>();
  const admits = (written: MockLink): boolean => {
    const file = indexOf(corpus).files.get(written.path);
    if (file === undefined || asked.has(written.path)) {
      return true;
    }
    const live = !file.node.generated && !file.node.archived;
    const admitted = live || (file.node.archived && !file.node.generated && answer.archive);
    if (!admitted) {
      leftOutEdges.set(written, file);
    }
    return admitted;
  };

  const index = graphIndexOf(corpus);
  const distance = new Map<MockNode, number>();
  const queue: MockNode[] = [];
  for (const holder of [...resolved.holders].sort(byPlace)) {
    if (!distance.has(holder)) {
      distance.set(holder, 0);
      queue.push(holder);
    }
  }
  const met = new Set<MockLink>();
  for (let next = 0; next < queue.length; next += 1) {
    const at = queue[next];
    if (at === undefined) {
      break;
    }
    const reached = distance.get(at) ?? 0;
    if (options.depth !== undefined && reached >= options.depth) {
      continue;
    }
    for (const node of nested(corpus, at)) {
      const out = (index.from.get(node) ?? [])
        .filter((link) => follow(link.written.type) === "out")
        .map((link) => [link, link.targets] as const);
      const back = (index.into.get(node) ?? [])
        .filter((link) => follow(link.written.type) === "in")
        .map((link) => [link, [link.source]] as const);
      for (const [link, far] of [...out, ...back]) {
        if (!admits(link.written)) {
          continue;
        }
        met.add(link.written);
        for (const reachedNode of far) {
          if (!distance.has(reachedNode)) {
            distance.set(reachedNode, reached + 1);
            queue.push(reachedNode);
          }
        }
      }
    }
  }

  const leftOut: LeftOut = { generated: 0, tier3: 0 };
  for (const file of leftOutEdges.values()) {
    if (file.node.generated) {
      leftOut.generated += 1;
    } else {
      leftOut.tier3 += 1;
    }
  }
  const nodes: GraphNode[] = [...distance.entries()]
    .sort(([a, da], [b, db]) => da - db || byPlace(a, b))
    .map(([node, at]) => ({
      id: node.id,
      kind: node.kind,
      title: node.title,
      path: node.path,
      line: node.line,
      distance: at,
      archived: node.archived,
    }));
  const edges: GraphEdge[] = corpus.links
    .filter((written) => met.has(written))
    .map((written) => {
      const source = holderOfLine(corpus, written.path, written.line);
      return {
        src: source === null ? null : nameOf(source),
        type: written.type,
        dst: written.state === "resolved" ? written.to : null,
        written: written.written,
        path: written.path,
        line: written.line,
        state: written.state,
        reason: written.reason,
      };
    })
    .sort(
      (a, b) =>
        compareText(a.type, b.type) || compareText(a.path, b.path) || a.line - b.line || compareText(a.written, b.written),
    );
  return { ...answer, reason: null, types, left_out: leftOut, nodes, edges };
}
