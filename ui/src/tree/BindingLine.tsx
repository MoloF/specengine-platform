import type { BindingPlace, BindingRow } from "../api/types";
import { Badge } from "../ui/Badge";
import { syncLook } from "./labels";

// One code binding row as lines (`docs/canon/code-reads.md` "The binding row"), shared by the
// Bundle tab's layers 5 and 9 (docs/features/bundle-bindings.md "UI and daemon keys") and the
// Bindings tab (docs/features/ui-node-bindings.md). Every word is shown raw or through the tree's
// labels; nothing in a row is a link: its node may be unresolved and its symbol is a qpath.

/** A row's place as `spec show --bindings` prints it: `<file>:<line>-<end_line>`, `<file>:<line>` or `<file>`. */
export function placeOf(row: BindingRow): string {
  if (row.line === null) {
    return row.file;
  }
  return row.end_line === null ? `${row.file}:${String(row.line)}` : `${row.file}:${String(row.line)}-${String(row.end_line)}`;
}

/** A binding as verify names it: `<kind> <symbol>[ <item_kind>]` (a keyless row has none: "No symbol"). */
export function bindingOf(place: Pick<BindingPlace, "kind" | "symbol" | "item_kind">): string {
  return place.item_kind === null ? `${place.kind} ${place.symbol}` : `${place.kind} ${place.symbol} ${place.item_kind}`;
}

/**
 * One binding row as lines: its node and relation, its place; its binding (keyless: "No symbol"),
 * layer and, in a bundle, tokens; its sync (verify's state, labelled, a row not judged "Not judged",
 * never "In sync"; category and detail raw); its signature; where it moved from or might be.
 * `noId` names a bare marker's `""` node. `focusable`: the line takes focus from "Show all".
 */
export function BindingLine({ row, tokens = null, noId, focusable = false }: { row: BindingRow; tokens?: number | null; noId: string; focusable?: boolean }) {
  const judgement = [row.category, row.detail].filter((word) => word !== null).join(" ");
  const key = row.kind !== null && row.symbol !== null ? bindingOf({ kind: row.kind, symbol: row.symbol, item_kind: row.item_kind }) : null;
  const from = row.anchor?.from ?? null;
  const candidates = row.anchor?.candidates ?? [];
  return (
    <li className="bundle-item binding-line" tabIndex={focusable ? -1 : undefined}>
      <p className="bundle-item-head">
        {row.node === "" ? <span>{noId}</span> : <bdi className="mono">{row.node}</bdi>}
        <span className="kind-tag">{row.relation}</span>
        <span className="mono">{placeOf(row)}</span>
      </p>
      <p className="bundle-item-meta">
        {key === null ? <span>No symbol</span> : <span className="mono">{key}</span>}
        {row.layer !== null && <span>layer {row.layer}</span>}
        {tokens !== null && <span>{tokens} tokens</span>}
      </p>
      <p className="bundle-item-meta">
        <Badge name="Sync" look={syncLook(row.sync)} />
        {judgement !== "" && <span className="mono">{judgement}</span>}
      </p>
      {row.signature !== null && (
        <p className="bundle-item-meta">
          <span>Signature:</span>
          <span className="mono">{row.signature}</span>
        </p>
      )}
      {from !== null && (
        <p className="bundle-item-meta">
          <span>Moved from</span>
          <span className="mono">
            {from.file} | {bindingOf(from)}
          </span>
        </p>
      )}
      {candidates.map((candidate) => (
        <p key={`${candidate.file}|${bindingOf(candidate)}`} className="bundle-item-meta">
          <span>Candidate</span>
          <span className="mono">
            {candidate.file} | {bindingOf(candidate)}
          </span>
        </p>
      ))}
    </li>
  );
}
