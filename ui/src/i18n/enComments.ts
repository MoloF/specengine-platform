// The English of the comments (docs/features/comments.md "UI"; ADR-0087): a part of the catalog
// that loads with the comments' chunks, never with the app, so first load holds only the four words
// drawn before them (`app.mentions`, `app.comments`, `app.lazy.comments`, `app.lazy.mentions`). Each
// chunk imports this module, which merges it into the instance before any view of it renders; the
// Russian stays whole in ru.ts, itself a chunk. Typed with `en` as the catalog (catalog.ts `English`).
import { addEnglishPart } from "./index";

export const enComments = {
  titleCount_one: "Comments · {{count}}",
  titleCount_other: "Comments · {{count}}",
  about: "Talk, not truth: a comment changes nothing until someone turns it into a question, a decision or a discrepancy.",
  failed: "The comments could not be read",
  notes: "Notes from the daemon",
  none: "No comments yet.",
  older: "Load older",
  loadingOlder: "Loading older…",
  new: "New comment",
  send: "Comment",
  sending: "Posting…",
  save: "Save",
  saving: "Saving…",
  reply: "Reply",
  replyLabel: "Reply to {{id}}",
  edit: "Edit",
  editLabel: "Edit {{id}}",
  acts: "Acts on {{id}}",
  view: "Show",
  write: "Write",
  preview: "Preview",
  previewEmpty: "Nothing to preview.",
  as: "Posted as {{name}}",
  blank: "Write a comment first.",
  stale: "Changed since you opened it: the version now stored stands above your text, which is kept; Save goes on it.",
  stored: "Now stored, version {{version}}:",
  agent: "agent",
  edited: "edited {{at}}, v{{version}}",
  onRev: "on rev {{rev}}",
  replyTo: "reply to {{id}}",
  deleted: "Deleted by {{by}}, {{at}}",
  items: "Items raised from it",
  said: { added: "Commented {{id}}", replied: "Replied {{id}}", edited: "Saved {{id}}, version {{version}}", deleted: "Deleted {{id}}" },
  delete: {
    action: "Delete…",
    title: "Delete {{id}}?",
    text: "The text of every version of {{who}}'s comment is erased; the thread keeps who deleted it and when.",
    submit: "Delete",
    sending: "Deleting…",
  },
  turn: {
    action: "Turn into…",
    menu: "Turn {{id}} into",
    discrepancy: "Report a discrepancy…",
    askTitle: "Ask a question from {{id}}",
    decideTitle: "Record a decision from {{id}}",
    reportTitle: "Report a discrepancy from {{id}}",
    report: "Report",
    reportAnyway: "Report anyway",
    reportAgain: "Report again",
    reporting: "Reporting…",
    reported: "Reported {{id}}",
    needsTexts: "Fill in the summary, what is observed, what is documented and the gap.",
    choose: "Choose…",
  },
  // Mentions, apart from the Inbox (docs/features/comments.md "UI", Mentions).
  mentions: {
    failed: "The mentions could not be read",
    about: "Comments naming {{login}}. This browser keeps which you have seen; the server keeps nothing.",
    aboutEvery: "Every comment naming a login: type your login as the acting name on the Board to see yours.",
    none: "No mentions",
    noneNext: "A comment that names you as @login shows here.",
    on: "on {{target}}",
    unseen_one: "{{count}} unseen",
    unseen_other: "{{count}} unseen",
  },
} as const;

addEnglishPart({ comments: enComments });
