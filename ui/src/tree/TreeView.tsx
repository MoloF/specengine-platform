import { useCallback, useEffect, useId, useMemo, useRef, useState, type KeyboardEvent, type MouseEvent, type Ref, type SubmitEvent } from "react";
import { apiErrorOf } from "../api/client";
import { useInbox, useNode, useSearch, useTree } from "../api/queries";
import type { SearchHit, TreeView as TreeDocument } from "../api/types";
import { movesHere, pushHash, useHash } from "../app/location";
import { RegionBoundary } from "../app/RegionBoundary";
import { sectionHash } from "../app/routes";
import { useOpenShortcuts } from "../app/shortcuts";
import { DEFAULT_TEXT_MODE, type TextMode } from "../markdown/TextMode";
import { useAnnounce } from "../ui/announcer";
import { Icon } from "../ui/Icon";
import { hasModifier, isTextField } from "../ui/keys";
import { ErrorPanel, Skeleton } from "../ui/states";
import { useFocusLater } from "../ui/useFocusLater";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { leftOutText } from "./LinksPanel";
import { countFor, targetIndex } from "./matching";
import { NoNodePane, NodePane, type Arrival } from "./NodePane";
import { buildRows, rowName, rowsForRef, type TreeRow } from "./rows";
import { hitCount, SearchResultsList } from "./SearchResults";
import { SpecTree } from "./SpecTree";

/** Below the tree: the daemon's notes, what was left out, a cut and how to narrow it. */
function TreeFooter({
  tree,
  archive,
  rooted,
  narrowTo,
  onRoot,
}: {
  tree: TreeDocument;
  archive: boolean;
  rooted: boolean;
  narrowTo: string | null;
  onRoot: (root: string | null) => void;
}) {
  const leftOut = leftOutText(tree.left_out);
  return (
    <div className="tree-foot">
      {tree.truncated && (
        <div className="notice cut-notice" role="note">
          <p className="notice-title">
            <Icon name="info" />
            <span>Showing the first {tree.nodes.length} nodes: the list was cut</span>
          </p>
          {narrowTo !== null && (
            <button
              type="button"
              className="button"
              onClick={() => {
                onRoot(narrowTo);
              }}
            >
              Show <span className="mono">{narrowTo}</span> as root
            </button>
          )}
        </div>
      )}
      {rooted && (
        <button
          type="button"
          className="button button-quiet"
          onClick={() => {
            onRoot(null);
          }}
        >
          Whole tree
        </button>
      )}
      {tree.notes.length > 0 && (
        <ul className="note-list" aria-label="Notes from the daemon">
          {tree.notes.map((note) => (
            <li key={note}>{note}</li>
          ))}
        </ul>
      )}
      {leftOut !== null && (
        <p className="note">
          <Icon name="info" />
          <span>
            {leftOut}
            {tree.left_out.tier3 > 0 && !archive && ". Include archive to show the archived ones."}
          </span>
        </p>
      )}
    </div>
  );
}

/**
 * The search form. The typed query is its own state: a keystroke renders this form alone, never
 * the tree or the node pane; the view learns the query on submit only. Esc in the field goes back
 * to the tree while hits are shown.
 */
function SearchField({
  inputRef,
  showHits,
  onSearch,
  onBack,
}: {
  inputRef: Ref<HTMLInputElement>;
  showHits: boolean;
  onSearch: (query: string) => void;
  onBack: () => void;
}) {
  const searchId = useId();
  const [query, setQuery] = useState("");
  const [queryError, setQueryError] = useState<string | null>(null);

  function submit(event: SubmitEvent<HTMLFormElement>) {
    event.preventDefault();
    if (query.trim() === "") {
      setQueryError("Type what to search for: words of three or more letters.");
      return;
    }
    setQueryError(null);
    onSearch(query);
  }

  return (
    <form role="search" aria-label="Spec search" className="tree-search" onSubmit={submit} noValidate>
      <label className="field-label" htmlFor={searchId}>
        Search the spec
      </label>
      <div className="search-row">
        <input
          id={searchId}
          ref={inputRef}
          className="field"
          type="search"
          autoComplete="off"
          spellCheck={false}
          placeholder="Words of three or more letters"
          value={query}
          aria-invalid={queryError !== null}
          aria-describedby={queryError === null ? undefined : `${searchId}-error`}
          onChange={(event) => {
            setQuery(event.target.value);
            setQueryError(null);
          }}
          onKeyDown={(event) => {
            if (event.key === "Escape" && showHits) {
              event.preventDefault();
              onBack();
            }
          }}
        />
        <button type="submit" className="button">
          <Icon name="search" />
          <span>Search</span>
        </button>
      </div>
      {queryError !== null && (
        <p id={`${searchId}-error`} className="field-error" role="alert">
          {queryError}
        </p>
      )}
    </form>
  );
}

/**
 * The Spec tree view (`#/<project>/tree[/<REF>]`): the containment tree with search on the left,
 * the node the route names on the right; stacked on narrow screens. Every region reads, fails
 * and retries on its own. Read-only: nothing here writes the spec.
 */
export function TreeView({ project, nodeRef }: { project: string; nodeRef: string | null }) {
  const openShortcuts = useOpenShortcuts();
  const announce = useAnnounce();
  const focusLater = useFocusLater();
  const hash = useHash();
  const archiveId = useId();
  const [archive, setArchive] = useState(false);
  // Rendered or Source, kept from node to node (docs/features/ui-markdown.md AC-09).
  const [textMode, setTextMode] = useState<TextMode>(DEFAULT_TEXT_MODE);
  const [root, setRoot] = useState<string | null>(null);
  const [submitted, setSubmitted] = useState<string | null>(null);
  const [showHits, setShowHits] = useState(false);

  const tree = useTree(project, root === null ? { archive } : { root, archive });
  const treeFailure = useRetainedFailure(tree.error, tree.isFetching);
  const node = useNode(project, nodeRef ?? "", {}, nodeRef !== null);
  const inbox = useInbox(project);
  const search = useSearch(project, submitted === null ? null : { query: submitted, archive });
  const searchFailure = useRetainedFailure(search.error, search.isFetching);

  const searchField = useRef<HTMLInputElement>(null);
  const activeRow = useRef<HTMLElement | null>(null);
  const treeRegion = useRef<HTMLDivElement>(null);
  const hitsRegion = useRef<HTMLDivElement>(null);
  const treeRetried = useRetryFocus(tree.data, () => activeRow.current ?? treeRegion.current);
  const searchRetried = useRetryFocus(search.data, () => hitsRegion.current);
  /** How the next node pane was reached: a followed link focuses its heading. */
  const intent = useRef<Arrival>("start");
  /** The hash last seen, so that a change of it ends a follow. */
  const seenHash = useRef(hash);
  /** The search answer last spoken: each answer is spoken once. */
  const spoken = useRef(0);

  const rows = useMemo(() => buildRows(tree.data?.nodes ?? []), [tree.data]);
  // The holders of the route's node once read; a previous node's answer kept up meanwhile is not it.
  const holders = nodeRef === null || node.data === undefined || node.isPlaceholderData ? null : node.data.nodes;
  const currentRows = useMemo(() => (nodeRef === null ? [] : rowsForRef(rows, nodeRef, holders)), [rows, nodeRef, holders]);
  const current = useMemo(() => currentRows.map((row) => row.key), [currentRows]);
  const counts = useMemo(() => {
    if (inbox.data === undefined) {
      return null;
    }
    const index = targetIndex(inbox.data.proposals);
    return new Map(rows.map((row) => [row.key, countFor(index, row.node)]));
  }, [inbox.data, rows]);

  const takeArrival = useCallback((): Arrival => {
    const now = intent.current;
    intent.current = "change";
    return now;
  }, []);
  // Only a link the page follows here sets it: a Cmd- or Ctrl-click opens elsewhere and a link to
  // the open hash moves nothing, so neither leaves it for the next row opened with Enter.
  const followed = useCallback((event: MouseEvent<HTMLAnchorElement>) => {
    if (movesHere(event)) {
      intent.current = "follow";
    }
  }, []);
  const openRow = useCallback(
    (row: TreeRow) => {
      pushHash(sectionHash(project, "tree", rowName(row.node)));
    },
    [project],
  );

  // A hash change ends a follow. The pane it mounts took it already (a child's effect runs before
  // its parent's); a change that mounts none (the same node by another spelling) leaves none.
  useEffect(() => {
    if (seenHash.current !== hash) {
      seenHash.current = hash;
      intent.current = "change";
    }
  }, [hash]);
  const setActiveRow = useCallback((element: HTMLElement | null) => {
    activeRow.current = element;
  }, []);

  // Each answer to a submitted search is counted once in the polite live region.
  const answered = search.dataUpdatedAt;
  const results = search.data;
  useEffect(() => {
    if (!showHits || results === undefined || answered === 0 || spoken.current === answered) {
      return;
    }
    spoken.current = answered;
    announce(hitCount(results));
  }, [answered, showHits, results, announce]);

  function open(name: string) {
    pushHash(sectionHash(project, "tree", name));
  }

  function runSearch(query: string) {
    setShowHits(true);
    if (submitted === query) {
      void search.refetch({ cancelRefetch: false });
    } else {
      setSubmitted(query);
    }
  }

  function backToTree() {
    setShowHits(false);
    focusLater(() => activeRow.current ?? searchField.current);
  }

  function onViewKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.defaultPrevented || hasModifier(event) || isTextField(event.target)) {
      return;
    }
    if (event.key === "?") {
      event.preventDefault();
      openShortcuts();
    }
  }

  let treeBody;
  const narrowTo = currentRows[0] ?? rows[0];
  if (tree.data === undefined) {
    treeBody =
      treeFailure === null ? (
        <div aria-busy="true">
          <Skeleton label={`Loading the spec tree of ${project}`} lines={8} />
        </div>
      ) : (
        <ErrorPanel
          title="The spec tree could not be loaded"
          message={apiErrorOf(treeFailure).message}
          retrying={tree.isFetching}
          attempt={tree.errorUpdateCount}
          onRetry={() => {
            treeRetried();
            void tree.refetch({ cancelRefetch: false });
          }}
        />
      );
  } else if (tree.data.reason !== null) {
    treeBody = (
      <div className="empty-state">
        <p>No tree from that root:</p>
        <p className="verbatim">{tree.data.reason}</p>
        <TreeFooter tree={tree.data} archive={archive} rooted={root !== null} narrowTo={null} onRoot={setRoot} />
      </div>
    );
  } else if (rows.length === 0) {
    treeBody = (
      <div className="empty-state">
        <h2>No document in the tree</h2>
        {tree.data.notes.map((note) => (
          <p key={note} className="verbatim">
            {note}
          </p>
        ))}
        <p className="muted">
          Next: write a spec document under the project&rsquo;s spec path; the tree shows it on the next read.
          {!archive && tree.data.left_out.tier3 > 0 && " Or include the archive."}
        </p>
        {root !== null && <TreeFooter tree={{ ...tree.data, notes: [] }} archive={archive} rooted narrowTo={null} onRoot={setRoot} />}
      </div>
    );
  } else {
    treeBody = (
      <>
        {root !== null && (
          <p className="tree-rooted">
            Rooted at <span className="mono">{root}</span>
          </p>
        )}
        <SpecTree rows={rows} current={current} counts={counts} onOpen={openRow} activeRef={setActiveRow} />
        <TreeFooter
          tree={tree.data}
          archive={archive}
          rooted={root !== null}
          narrowTo={narrowTo === undefined ? null : rowName(narrowTo.node)}
          onRoot={setRoot}
        />
      </>
    );
  }

  let hits = null;
  if (showHits) {
    if (searchFailure !== null && search.data === undefined) {
      hits = (
        <ErrorPanel
          title="The search could not run"
          message={apiErrorOf(searchFailure).message}
          retrying={search.isFetching}
          attempt={search.errorUpdateCount}
          onRetry={() => {
            searchRetried();
            void search.refetch({ cancelRefetch: false });
          }}
        />
      );
    } else if (search.data === undefined) {
      hits = (
        <div aria-busy="true">
          <Skeleton label={`Searching for ${submitted ?? ""}`} lines={5} />
        </div>
      );
    } else {
      hits = (
        <div ref={hitsRegion} tabIndex={-1} aria-busy={search.isFetching}>
          <SearchResultsList
            key={`${search.data.query}|${String(answered)}`}
            results={search.data}
            archive={archive}
            onOpen={(hit: SearchHit) => {
              open(rowName(hit));
            }}
            onBack={backToTree}
          />
          {search.error !== null && (
            <p className="note" role="alert">
              The search could not run again: {apiErrorOf(search.error).message}
            </p>
          )}
        </div>
      );
    }
  }

  return (
    <div className="tree-view" onKeyDown={onViewKeyDown}>
      <div className="tree-pane">
        <SearchField inputRef={searchField} showHits={showHits} onSearch={runSearch} onBack={backToTree} />
        <div className="archive-toggle">
          <input
            id={archiveId}
            type="checkbox"
            checked={archive}
            aria-describedby={`${archiveId}-hint`}
            onChange={(event) => {
              setArchive(event.target.checked);
            }}
          />
          <label htmlFor={archiveId}>Include archive</label>
          <span id={`${archiveId}-hint`} className="sr-only">
            Archived documents in the tree and the search, and the links written in them.
          </span>
        </div>
        {showHits && (
          <section className="search-region" aria-label="Search results">
            <RegionBoundary name="search results">{hits}</RegionBoundary>
          </section>
        )}
        <div ref={treeRegion} tabIndex={-1} className="tree-region" hidden={showHits} aria-busy={tree.data !== undefined && tree.isFetching}>
          <RegionBoundary name="spec tree">{treeBody}</RegionBoundary>
        </div>
      </div>
      <div className="node-column">
        <RegionBoundary key={nodeRef ?? ""} name="node">
          {nodeRef === null ? (
            <NoNodePane />
          ) : (
            <NodePane
              key={`${project}/${nodeRef}`}
              project={project}
              nodeRef={nodeRef}
              archive={archive}
              rows={rows}
              inbox={inbox}
              textMode={textMode}
              onTextMode={setTextMode}
              arrival={takeArrival}
              onFollow={followed}
            />
          )}
        </RegionBoundary>
      </div>
    </div>
  );
}
