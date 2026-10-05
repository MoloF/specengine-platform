import { useMemo, type MouseEvent } from "react";
import type { ShownNode } from "../api/types";
import { sectionHash } from "../app/routes";
import { Icon } from "../ui/Icon";
import { placeKey, rowName, type TreeRow } from "./rows";
import { splitLines, TextView, type LineTarget } from "./TextView";

/** A jump into one holder's text. */
export interface TextTarget extends LineTarget {
  /** The holder's place (`path:line`). */
  place: string;
}

/**
 * The line a nested section's heading sits on: its tree row's line when the tree lists it, else
 * the first heading line in the text naming it; null when neither says.
 */
export function sectionLine(holder: ShownNode, section: string, rows: readonly TreeRow[]): number | null {
  const row = rows.find(
    ({ node }) => node.id === section && node.path === holder.path && node.line >= holder.line && node.line <= holder.end_line,
  );
  if (row !== undefined) {
    return row.node.line;
  }
  const at = splitLines(holder.text).findIndex((line) => /^#{1,6}\s/.test(line) && line.includes(section));
  return at < 0 ? null : holder.line + at;
}

function names(list: string[], more: number): string {
  return more > 0 ? `${list.join(", ")}, ${String(more)} more` : list.join(", ");
}

/** One holder's text: its sections to jump to, what a cut left out, the numbered text. */
function HolderText({
  project,
  holder,
  rows,
  many,
  target,
  onJump,
  onFollow,
}: {
  project: string;
  holder: ShownNode;
  rows: readonly TreeRow[];
  many: boolean;
  target: TextTarget | null;
  onJump: (place: string, line: number) => void;
  onFollow: (event: MouseEvent<HTMLAnchorElement>) => void;
}) {
  const place = placeKey(holder);
  // One scan of the rows per holder and tree read, not one per section on every render.
  const sectionLines = useMemo(
    () => new Map(holder.sections.map((section) => [section, sectionLine(holder, section, rows)])),
    [holder, rows],
  );
  const range = `${holder.path}:${String(holder.line)}-${String(holder.end_line)}`;
  const omitted = holder.omitted;
  return (
    <section className="holder-text" aria-label={many ? `Text at ${range}` : undefined}>
      {many && <h2 className="holder-text-title mono">{range}</h2>}
      {holder.sections.length > 0 && (
        <nav className="section-jumps" aria-label={`Sections of ${rowName(holder)}`}>
          <span className="section-jumps-label">Sections</span>
          <ul>
            {holder.sections.map((section) => {
              const line = sectionLines.get(section) ?? null;
              return (
                <li key={section}>
                  {line === null ? (
                    <span className="mono">{section}</span>
                  ) : (
                    <button
                      type="button"
                      className="link-button mono"
                      onClick={() => {
                        onJump(place, line);
                      }}
                    >
                      {section}
                      <span className="sr-only">, line {line}</span>
                    </button>
                  )}
                </li>
              );
            })}
          </ul>
        </nav>
      )}
      {holder.truncated && omitted !== null && (
        <div className="notice cut-notice" role="note">
          <p className="notice-title">
            <Icon name="info" />
            <span>
              Lines {omitted.lines[0]}-{omitted.lines[1]} not shown
            </span>
          </p>
          {omitted.sections.length > 0 && (
            <p>
              Sections not shown:{" "}
              {omitted.sections.map((section, index) => (
                <span key={section}>
                  {index > 0 && ", "}
                  <a className="mono" href={sectionHash(project, "tree", section)} onClick={onFollow}>
                    {section}
                  </a>
                </span>
              ))}
              {omitted.sections_more > 0 && `, ${String(omitted.sections_more)} more`}
            </p>
          )}
          {omitted.holders.length > 0 && (
            <p>
              Holders not shown: <span className="mono">{names(omitted.holders, omitted.holders_more)}</span>
            </p>
          )}
        </div>
      )}
      {!holder.utf8 && (
        <p className="note">
          <Icon name="info" />
          <span>This file is not UTF-8: undecodable bytes show as replacement characters.</span>
        </p>
      )}
      <TextView
        text={holder.text}
        firstLine={holder.line}
        label={`Text of ${rowName(holder)}, lines ${String(holder.line)} to ${String(holder.end_line)}`}
        target={target !== null && target.place === place ? target : null}
      />
    </section>
  );
}

/** The Text tab: each holder's text verbatim, numbered from its first line. */
export function TextPanel({
  project,
  holders,
  rows,
  target,
  onJump,
  onFollow,
}: {
  project: string;
  holders: readonly ShownNode[];
  rows: readonly TreeRow[];
  target: TextTarget | null;
  onJump: (place: string, line: number) => void;
  onFollow: (event: MouseEvent<HTMLAnchorElement>) => void;
}) {
  return (
    <div className="text-panel">
      {holders.map((holder) => (
        <HolderText
          key={placeKey(holder)}
          project={project}
          holder={holder}
          rows={rows}
          many={holders.length > 1}
          target={target}
          onJump={onJump}
          onFollow={onFollow}
        />
      ))}
    </div>
  );
}
