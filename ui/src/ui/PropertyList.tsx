import type { ReactNode } from "react";
import { useTranslate } from "../i18n";
import { Disclosure } from "./Disclosure";

// A list of properties (docs/features/design-foundation.md "Primitives (`ui/src/ui/`)"): a `dl` of label and value
// rows, the labels 12/16 secondary, the values 14/20, in the caller's order. With `foldEmpty`, a row
// whose value is null leaves the list and stands, in the same order, under one "More fields (n)"
// disclosure, collapsed, each reading "None"; without it an empty value reads "None" in place.

export interface Property {
  key: string;
  label: string;
  value: ReactNode | null;
}

function Row({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div className="property-row">
      <dt className="property-label">{label}</dt>
      <dd className="property-value">{value}</dd>
    </div>
  );
}

export function PropertyList({ label, items, foldEmpty = false }: { label: string; items: readonly Property[]; foldEmpty?: boolean }) {
  const t = useTranslate();
  const none = <span className="property-empty">{t("ui.emptyValue")}</span>;
  const shown = foldEmpty ? items.filter((item) => item.value !== null) : items;
  const folded = foldEmpty ? items.filter((item) => item.value === null) : [];
  return (
    <section className="property-list" aria-label={label}>
      <dl className="property-rows">
        {shown.map((item) => (
          <Row key={item.key} label={item.label} value={item.value ?? none} />
        ))}
      </dl>
      {folded.length > 0 && (
        <Disclosure summary={t("ui.moreFields", { n: String(folded.length) })}>
          <dl className="property-rows">
            {folded.map((item) => (
              <Row key={item.key} label={item.label} value={none} />
            ))}
          </dl>
        </Disclosure>
      )}
    </section>
  );
}
