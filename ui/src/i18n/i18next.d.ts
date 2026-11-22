// The catalog types `t` (docs/features/ui-languages.md "Data", LG-9): a key `en` lacks, or a
// placeholder it does not name, fails `tsc --noEmit`.
import "i18next";
import type { English } from "./catalog";

declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: "translation";
    // `en` and the parts that load with their views' chunks (catalog.ts `English`).
    resources: { translation: English };
  }
}
