// Fixed `t`s for tests (docs/features/ui-languages.md "Data", "The instance (`index.ts`)"): a label
// function called with `enT` gives today's English, with `ruT` the Russian of ru.ts, whatever the
// page shows. Tests reach Russian through these or ru.ts, never a Cyrillic literal (D13).
import { translateIn } from "../i18n";

export const enT = translateIn("en");
export const ruT = translateIn("ru");
