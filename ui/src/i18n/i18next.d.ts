// The catalog types `t` (docs/features/ui-languages.md "Data", LG-9): a key `en` lacks, or a
// placeholder it does not name, fails `tsc --noEmit`.
import "i18next";
import type { en } from "./en";

declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: "translation";
    resources: { translation: typeof en };
  }
}
