// The catalogs' type, as docs/canon/ui-languages.md "Catalog" lays it out: English is the source of
// the keys, Russian is typed against it, so a Russian key missing or added fails `tsc`. No string is
// resolved here, and no Russian value is imported: ru.ts is a chunk of its own, reached only through
// index.ts's `import("./ru")`.
import type { en } from "./en";

/** The chrome's languages: a third one is a new decision (ADR-0072). */
export type Language = "en" | "ru";

export const LANGUAGES: readonly Language[] = ["en", "ru"];

export function isLanguage(value: string): value is Language {
  return (LANGUAGES as readonly string[]).includes(value);
}

/** The Russian forms a plural group gains beside English's `_one` and `_other`. */
type RussianForms<K> = K extends `${infer Base}_other` ? `${Base}_few` | `${Base}_many` : never;

/** `en`'s shape with every leaf a string, every plural group gaining `_few` and `_many`. */
type Shaped<T> = {
  readonly [K in keyof T | RussianForms<keyof T>]: K extends keyof T ? (T[K] extends string ? string : Shaped<T[K]>) : string;
};

/** A catalog of the chrome: what `ru.ts` is typed as. */
export type Catalog = Shaped<typeof en>;

/** Any catalog as plain nested text, English's included: what the parity test reads. */
export interface Messages {
  readonly [key: string]: string | Messages;
}
