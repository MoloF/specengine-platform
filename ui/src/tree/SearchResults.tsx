import { useRef, useState, type KeyboardEvent } from "react";
import type { SearchHit, SearchResults as Results, Snippet } from "../api/types";
import { Icon } from "../ui/Icon";
import { hasModifier, isTextField } from "../ui/keys";
import { useFocusLater } from "../ui/useFocusLater";
import { rowName } from "./rows";

/** A snippet as text: hits in <mark>, a cut end as an ellipsis; never markup, never `**` parsed. */
export function SnippetText({ snippet }: { snippet: Snippet }) {
  return (
    <span className="snippet">
      {snippet.cut_start && <span aria-hidden="true">…</span>}
      {snippet.segments.map((segment, index) =>
        segment.hit ? <mark key={index}>{segment.text}</mark> : <span key={index}>{segment.text}</span>,
      )}
      {snippet.cut_end && <span aria-hidden="true">…</span>}
    </span>
  );
}

function HitContent({ hit }: { hit: SearchHit }) {
  return (
    <>
      <span className="hit-head">
        <span className="mono hit-name">{rowName(hit)}</span>
        <span className="kind-tag">
          <span className="sr-only">Kind: </span>
          {hit.kind ?? "-"}
        </span>
        {hit.title !== null && <span className="hit-title">{hit.title}</span>}
      </span>
      <span className="hit-meta">
        <span className="mono">
          {hit.path}:{hit.line}
        </span>
        {hit.archived && (
          <span className="plain-tag">
            <Icon name="archived" />
            archived
          </span>
        )}
      </span>
      {hit.snippet !== null && <SnippetText snippet={hit.snippet} />}
    </>
  );
}

/** "<n> hits (limit <l>)": what the polite live region says too. */
export function hitCount(results: Results): string {
  const n = results.hits.length;
  return `${String(n)} ${n === 1 ? "hit" : "hits"} (limit ${String(results.limit)})`;
}

/**
 * The hits of a search, replacing the tree until Back or Esc: a listbox with one tab stop; Up and
 * Down (k, j), Home and End move, Enter or a click opens the hit's node and keeps focus here. The
 * list's keys act on the list alone: Enter on "Back to tree" is that button's. Esc anywhere here
 * goes back.
 */
export function SearchResultsList({
  results,
  archive,
  onOpen,
  onBack,
}: {
  results: Results;
  archive: boolean;
  onOpen: (hit: SearchHit) => void;
  onBack: () => void;
}) {
  const focusLater = useFocusLater();
  const list = useRef<HTMLDivElement>(null);
  const [active, setActive] = useState(0);
  const hits = results.hits;
  const at = Math.min(active, Math.max(0, hits.length - 1));

  function option(index: number): HTMLElement | null {
    return list.current?.querySelector<HTMLElement>(`[data-hit="${String(index)}"]`) ?? null;
  }

  function move(index: number) {
    if (index < 0 || index >= hits.length) {
      return;
    }
    setActive(index);
    focusLater(() => option(index));
  }

  function onListKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (hasModifier(event) || !(event.target instanceof Element) || event.target.closest('[role="option"]') === null) {
      return;
    }
    switch (event.key) {
      case "ArrowDown":
      case "j":
        move(at + 1);
        break;
      case "ArrowUp":
      case "k":
        move(at - 1);
        break;
      case "Home":
        move(0);
        break;
      case "End":
        move(hits.length - 1);
        break;
      case "Enter": {
        const hit = hits[at];
        if (hit !== undefined) {
          onOpen(hit);
        }
        break;
      }
      default:
        return;
    }
    event.preventDefault();
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.defaultPrevented || hasModifier(event) || isTextField(event.target) || event.key !== "Escape") {
      return;
    }
    event.preventDefault();
    onBack();
  }

  return (
    <div className="search-results" onKeyDown={onKeyDown}>
      <div className="search-results-head">
        <p className="search-query">
          Hits for <span className="verbatim">{results.query}</span>
        </p>
        <button type="button" className="button button-quiet" onClick={onBack}>
          <Icon name="back" />
          <span>Back to tree</span>
        </button>
      </div>
      {hits.length === 0 ? (
        <div className="empty-state">
          <p>
            No node matches &ldquo;<span className="verbatim">{results.query}</span>&rdquo;.
          </p>
          <p className="muted">
            Next: every word of three or more letters must occur in a node&rsquo;s title or text; try fewer or other words
            {!archive && ", or include archived documents"}.
          </p>
        </div>
      ) : (
        <div ref={list} role="listbox" aria-label={`Search hits for ${results.query}`} className="hit-list" onKeyDown={onListKeyDown}>
          {hits.map((hit, index) => (
            <div
              key={`${hit.path}:${String(hit.line)}`}
              role="option"
              aria-selected={index === at}
              tabIndex={index === at ? 0 : -1}
              data-hit={index}
              className="hit"
              onClick={() => {
                setActive(index);
                onOpen(hit);
              }}
            >
              <HitContent hit={hit} />
            </div>
          ))}
        </div>
      )}
      <div className="search-foot">
        <p className="search-count">{hitCount(results)}</p>
        {results.tier3_left_out > 0 && (
          <p className="note">
            <Icon name="archived" />
            <span>
              {results.tier3_left_out} archived {results.tier3_left_out === 1 ? "match" : "matches"} left out
              {!archive && ": Include archive to list them"}
            </span>
          </p>
        )}
        {results.notes.length > 0 && (
          <ul className="note-list" aria-label="Notes from the daemon">
            {results.notes.map((note) => (
              <li key={note}>{note}</li>
            ))}
          </ul>
        )}
        {results.truncated && (
          <p className="note">
            <Icon name="info" />
            <span>The list was cut at the output cap: narrow the query to see the rest.</span>
          </p>
        )}
      </div>
    </div>
  );
}
