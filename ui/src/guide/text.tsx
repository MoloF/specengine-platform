import { Fragment } from "react";
import { codeSpans } from "../markdown/cited";

// A guide title or summary as the daemon serves it: the page's own words in markdown, English, shown
// as written; its backtick code spans set as code, the backticks with them, nothing else read.

/** The words with each code span as `code`, its backticks dropped: "The `[auto]` mode" reads The [auto] mode. */
export function GuideWords({ text }: { text: string }) {
  const parts: { text: string; code: boolean }[] = [];
  let plain = 0;
  for (const [start, end] of codeSpans(text)) {
    const run = /^`+/.exec(text.slice(start))?.[0].length ?? 1;
    if (start > plain) {
      parts.push({ text: text.slice(plain, start), code: false });
    }
    parts.push({ text: text.slice(start + run, end - run), code: true });
    plain = end;
  }
  if (plain < text.length) {
    parts.push({ text: text.slice(plain), code: false });
  }
  return parts.map((part, index) => (part.code ? <code key={index}>{part.text}</code> : <Fragment key={index}>{part.text}</Fragment>));
}

/** The words as markdown reads them, without their backticks: the page's first heading compared with its title. */
export function plainWords(text: string): string {
  return text.replaceAll("`", "");
}
