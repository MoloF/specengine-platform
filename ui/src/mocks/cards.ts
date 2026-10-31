import { ClientError, CARD_REFUSED, CARD_USAGE, NO_SUCH_CARD, type CardsOptions } from "../api/client";
import type {
  Card,
  CardAcceptBody,
  CardAssignBody,
  CardCancelBody,
  CardCreateBody,
  CardEditBody,
  CardEvent,
  CardFlow,
  CardList,
  CardMoveBody,
  CardNotFound,
  CardRow,
  CardWrite,
} from "../api/types";
import { stamp } from "./build";
import type { Scenario } from "./scenario";

// The mock's cards (`docs/canon/cards.md`), in memory until a reload: two invented boards and the
// daemon's rules in its words, so the Board's refusals can be seen without one. harbor-sim prints an
// almanac, two trades on two flows, the plates' `blocked` a column like any other; ledger-api keeps
// the default flow, and one card a column left behind.

/** A stored card: the document less what a read adds. */
export type StoredCard = Omit<Card, "allowed" | "history" | "notes">;

interface MockEvent extends CardEvent {
  id: string;
}

/** One project's board state. */
export interface MockBoard {
  flows: CardFlow[];
  cards: StoredCard[];
  events: MockEvent[];
  seq: number;
}

const MINUTE_DAY = 24 * 60;

/** What `as` records when no name is sent. */
const LOCAL = "local";

/** How a cleared value is written. */
const CLEAR = "-";

/** At most this many bytes in a person's name, `CARD_PERSON_MAX`. */
const NAME_MAX_BYTES = 128;

/** The `conflict` scenario's other hand. */
const ELSEWHERE = "Dana Reyes";

/** Every ordered pair of the columns but `accept.to`, as the reader expands a flow with no `moves`. */
function everyMove(columns: readonly string[], closing: string) {
  const open = columns.filter((column) => column !== closing);
  return open.flatMap((from) => open.filter((to) => to !== from).map((to) => ({ from, to, by: [] })));
}

const COPY: CardFlow = {
  name: "copy",
  disciplines: ["copy"],
  columns: [
    { id: "draft", title: "Draft" },
    { id: "edit", title: "Edit" },
    { id: "proof", title: "Proof" },
    { id: "filed", title: "Filed" },
  ],
  moves: [
    { from: "draft", to: "edit", by: ["desk"] },
    { from: "edit", to: "proof", by: [] },
  ],
  accept: { from: "proof", to: "filed" },
};

const PLATES: CardFlow = {
  name: "plates",
  disciplines: ["plates"],
  columns: ["queued", "blocked", "checked", "struck"].map((id) => ({ id, title: id })),
  moves: everyMove(["queued", "blocked", "checked", "struck"], "struck"),
  accept: { from: "checked", to: "struck" },
};

const PLAIN: CardFlow = {
  name: "plain",
  disciplines: [],
  columns: [
    { id: "backlog", title: "Backlog" },
    { id: "ready", title: "Ready" },
    { id: "in-progress", title: "In progress" },
    { id: "review", title: "Review" },
    { id: "done", title: "Done" },
  ],
  moves: everyMove(["backlog", "ready", "in-progress", "review", "done"], "done"),
  accept: { from: "review", to: "done" },
};

/** A calendar day `days` from `at`, as `--due` writes one. */
function dayFrom(at: number, days: number): string {
  return stamp(at, -days * MINUTE_DAY).slice(0, 10);
}

/** A stored card; omitted fields as a new card has them. */
function aCard(at: number, fields: Partial<StoredCard> & Pick<StoredCard, "id" | "flow" | "column" | "position" | "title">): StoredCard {
  return {
    goal: null,
    discipline: null,
    targets: [],
    snapshot: null,
    criteria: [],
    assignee: null,
    reviewer: null,
    estimate: null,
    due: null,
    parent: null,
    corrects: null,
    evidence: [],
    acceptance: null,
    cancellation: null,
    created_by: "Mira Holt",
    created_at: stamp(at, 3 * MINUTE_DAY),
    updated_at: stamp(at, 2 * MINUTE_DAY),
    revision: "1",
    ...fields,
  };
}

function harborCards(at: number): StoredCard[] {
  return [
    aCard(at, {
      id: "C-0001",
      flow: "copy",
      column: "draft",
      position: 1,
      title: "Write the tide table foreword",
      goal: "One page a skipper reads before the first table: what the numbers mean and where they stop being true.",
      discipline: "copy",
      targets: ["MEC-TIDE-TABLES", "MEC-TIDES", "DOM-WATER"],
      criteria: ["Names the datum the tables use", "Says what a surge does to them"],
      assignee: "Mira Holt",
      reviewer: "Ivo Brandt",
      estimate: "3",
      due: dayFrom(at, -2),
    }),
    aCard(at, {
      id: "C-0002",
      flow: "copy",
      column: "draft",
      position: 2,
      title: "Caption the berth plans",
      discipline: "copy",
      targets: ["DOM-BERTHS"],
      estimate: "1-2",
      due: dayFrom(at, 6),
    }),
    aCard(at, {
      id: "C-0003",
      flow: "copy",
      column: "edit",
      position: 1,
      title: "Night passage rules, plain words",
      goal: "The rule as a pilot would say it over the radio.",
      discipline: "copy",
      targets: ["MEC-NIGHT-PASSAGE"],
      assignee: "Tomas Oakes",
      reviewer: "Ivo Brandt",
      estimate: "2",
      due: dayFrom(at, 1),
      evidence: [{ kind: "note", value: "Second read done with the pilots' office", by: "Tomas Oakes", at: stamp(at, 26 * 60) }],
      revision: "3",
    }),
    aCard(at, {
      id: "C-0004",
      flow: "copy",
      column: "proof",
      position: 1,
      title: "Proof the lock opening hours",
      discipline: "copy",
      targets: ["MEC-LOCK-A", "MEC-LOCK-B"],
      criteria: ["Every hour checked against the lock master's sheet"],
      assignee: "Mira Holt",
      reviewer: "Ivo Brandt",
      estimate: "1",
      due: dayFrom(at, 0),
      evidence: [{ kind: "url", value: "https://example.org/locks/hours-2026.pdf", by: "Mira Holt", at: stamp(at, 5 * 60) }],
      revision: "5",
    }),
    aCard(at, {
      id: "C-0005",
      flow: "copy",
      column: "filed",
      position: 1,
      title: "Mooring fees table",
      discipline: "copy",
      targets: ["MEC-MOORING"],
      assignee: "Tomas Oakes",
      reviewer: "Mira Holt",
      estimate: "1",
      due: dayFrom(at, -9),
      acceptance: { by: "Mira Holt", at: stamp(at, 4 * MINUTE_DAY), note: "Matches the harbour office's rates." },
      revision: "6",
    }),
    aCard(at, {
      id: "C-0006",
      flow: "copy",
      column: "draft",
      position: 0,
      title: "Advert for the old quays tour",
      discipline: "copy",
      assignee: "Lena Fisk",
      due: dayFrom(at, -20),
      cancellation: { by: "Ivo Brandt", at: stamp(at, 9 * MINUTE_DAY), reason: "The tour stops this season." },
      revision: "2",
    }),
    aCard(at, {
      id: "C-0007",
      flow: "plates",
      column: "queued",
      position: 1,
      title: "Fairway chart, sheet 2",
      discipline: "plates",
      targets: ["MEC-FAIRWAY"],
      assignee: "Noor Ainsley",
      reviewer: "Tomas Oakes",
      estimate: "4",
      due: dayFrom(at, 10),
    }),
    aCard(at, {
      id: "C-0008",
      flow: "plates",
      column: "blocked",
      position: 1,
      title: "Crane reach diagram",
      goal: "Waiting on the survey's new numbers; the plate is set otherwise.",
      discipline: "plates",
      targets: ["MEC-CARGO-CRANES"],
      assignee: "Noor Ainsley",
      estimate: "2",
    }),
    aCard(at, {
      id: "C-0009",
      flow: "plates",
      column: "checked",
      position: 1,
      title: "Harbour overview, colour plate",
      discipline: "plates",
      targets: ["DOM-HARBOR"],
      assignee: "Lena Fisk",
      reviewer: "Noor Ainsley",
      estimate: "3",
      due: dayFrom(at, 2),
      evidence: [{ kind: "build", value: "plate-run-41", by: "Lena Fisk", at: stamp(at, 8 * 60) }],
    }),
  ];
}

function ledgerCards(at: number): StoredCard[] {
  return [
    aCard(at, {
      id: "C-0101",
      flow: "plain",
      column: "backlog",
      position: 1,
      title: "Write the refund runbook",
      targets: ["SVC-REFUNDS"],
      estimate: "2",
    }),
    aCard(at, {
      id: "C-0102",
      flow: "plain",
      column: "ready",
      position: 1,
      title: "Publish the idempotency rule for partners",
      targets: ["POL-IDEMPOTENCY"],
      assignee: "Ana Weiss",
      due: dayFrom(at, 4),
    }),
    aCard(at, {
      id: "C-0103",
      flow: "plain",
      column: "in-progress",
      position: 1,
      title: "Book the finance review",
      assignee: "Omar Lind",
      reviewer: "Ana Weiss",
      due: dayFrom(at, -1),
    }),
    aCard(at, {
      id: "C-0104",
      flow: "plain",
      column: "review",
      position: 1,
      title: "Refund window, customer wording",
      targets: ["POL-REFUND-WINDOW", "POL-REFUND-RULES"],
      assignee: "Omar Lind",
      reviewer: "Ana Weiss",
      estimate: "1",
    }),
    aCard(at, {
      id: "C-0105",
      flow: "plain",
      column: "parked",
      position: 1,
      title: "Old settlement report",
      assignee: "Ana Weiss",
    }),
  ];
}

/** The `large` scenario's column: past the Board's cap. */
function largeColumn(at: number): StoredCard[] {
  return Array.from({ length: 230 }, (_, index) =>
    aCard(at, {
      id: `C-${String(1000 + index).padStart(4, "0")}`,
      flow: "copy",
      column: "edit",
      position: index + 2,
      title: `Chart note ${String(index + 1)}`,
      discipline: "copy",
    }),
  );
}

/** A project's board for a scenario; `empty` keeps the columns and no card. */
export function mockBoard(slug: string, scenario: Scenario, at: number): MockBoard {
  const harbor = slug === "harbor-sim";
  const flows = structuredClone(harbor ? [COPY, PLATES] : [PLAIN]);
  if (scenario === "empty") {
    return { flows, cards: [], events: [], seq: 0 };
  }
  const cards = [...(harbor ? harborCards(at) : ledgerCards(at)), ...(harbor && scenario === "large" ? largeColumn(at) : [])];
  const events = cards.map((card, index): MockEvent => ({
    id: card.id,
    seq: index + 1,
    type: "card.created",
    at: card.created_at,
    payload: { id: card.id, flow: card.flow, column: card.column, position: card.position, assignee: card.assignee, as: card.created_by },
  }));
  return { flows, cards, events, seq: events.length };
}

function flowOf(board: MockBoard, name: string): CardFlow | undefined {
  return board.flows.find((flow) => flow.name === name);
}

function closed(card: StoredCard): boolean {
  return card.acceptance !== null || card.cancellation !== null;
}

function holds(flow: CardFlow, column: string): boolean {
  return flow.columns.some((candidate) => candidate.id === column);
}

/** What a read says is open: a closed card none; a column the flow left, its first column. */
function allowedOf(board: MockBoard, card: StoredCard) {
  const flow = flowOf(board, card.flow);
  if (flow === undefined || closed(card)) {
    return [];
  }
  if (!holds(flow, card.column)) {
    return [{ to: flow.columns[0]?.id ?? card.column, by: [] }];
  }
  return flow.moves.filter((move) => move.from === card.column).map(({ to, by }) => ({ to, by: [...by] }));
}

function rowOf({ id, flow, column, position, title, discipline, targets, assignee, reviewer, estimate, due, parent, corrects, updated_at, revision, acceptance, cancellation }: StoredCard): CardRow {
  return { id, flow, column, position, title, discipline, targets, assignee, reviewer, estimate, due, parent, corrects, updated_at, revision, accepted: acceptance !== null, cancelled: cancellation !== null };
}

/** A card's place in the list: flow, column by config, cancelled last, position. */
function placeOf(board: MockBoard, card: StoredCard): readonly number[] {
  const flow = board.flows.findIndex((candidate) => candidate.name === card.flow);
  const columns = board.flows[flow]?.columns ?? [];
  const column = columns.findIndex((candidate) => candidate.id === card.column);
  return [flow < 0 ? board.flows.length : flow, column < 0 ? columns.length : column, card.cancellation === null ? 0 : 1, card.position];
}

function byPlace(board: MockBoard) {
  return (a: StoredCard, b: StoredCard): number => {
    const left = placeOf(board, a);
    const right = placeOf(board, b);
    const at = left.findIndex((value, index) => value !== right[index]);
    return at < 0 ? 0 : (left[at] ?? 0) - (right[at] ?? 0);
  };
}

/** `spec card list --json`, the browser view. */
export function cardListOf(board: MockBoard, options: CardsOptions = {}): CardList {
  const listed = board.cards.filter((card) => options.with_cancelled === true || card.cancellation === null).sort(byPlace(board));
  const stranded = new Map<string, StoredCard[]>();
  for (const card of listed) {
    const flow = flowOf(board, card.flow);
    if (flow !== undefined && !holds(flow, card.column)) {
      const key = `${card.flow}/${card.column}`;
      stranded.set(key, [...(stranded.get(key) ?? []), card]);
    }
  }
  const notes = Array.from(stranded, ([key, group]) => `${String(group.length)} card(s) in ${key}, from ${group[0]?.id ?? ""}: the column \`${group[0]?.column ?? ""}\` is no longer in the flow \`${group[0]?.flow ?? ""}\`: served last`);
  return { flows: structuredClone(board.flows), cards: listed.map(rowOf), notes, truncated: false, omitted: null };
}

/** `spec card show C --history --json`, uncut; an unknown C its exit-1 document. */
export function cardShowOf(board: MockBoard, id: string): Card | CardNotFound {
  const card = board.cards.find((candidate) => candidate.id === id);
  if (card === undefined) {
    return { id, reason: `no card ${id}` };
  }
  const history = board.events.filter((event) => event.id === id).map(({ seq, type, at, payload }) => ({ seq, type, at, payload }));
  return structuredClone({ ...card, allowed: allowedOf(board, card), history, notes: [] });
}

/** What cannot stand in a person's name: a control character, a line or paragraph separator, a bidirectional mark. */
const NOT_IN_A_NAME = /[\u0000-\u001f\u007f-\u009f\u061c\u200e\u200f\u2028\u2029\u202a-\u202e\u2066-\u2069]/u;

/** A person's name off its grammar: a 400 in the CLI's line, before anything is read. */
function checkPerson(key: string, name: string | null | undefined): void {
  if (name === null || name === undefined) {
    return;
  }
  const trimmed = name.trim();
  const bytes = new TextEncoder().encode(trimmed).length;
  const bad = NOT_IN_A_NAME.exec(trimmed)?.[0];
  if (bytes > NAME_MAX_BYTES) {
    throw new ClientError({ status: CARD_USAGE, message: `spec: ${key}: ${String(bytes)} bytes; at most ${String(NAME_MAX_BYTES)}` });
  }
  if (bad !== undefined) {
    const code = (bad.codePointAt(0) ?? 0).toString(16).toUpperCase().padStart(4, "0");
    throw new ClientError({ status: CARD_USAGE, message: `spec: ${key}: U+${code} in a name: one line, no control character or bidirectional mark` });
  }
}

/** The acting name, as the flag `--as` judges it. */
function checkName(as: string | null): void {
  checkPerson("--as", as);
}

function refused(reason: string): ClientError {
  return new ClientError({ status: CARD_REFUSED, message: reason });
}

/** The card a write is for: unknown 404, closed 409, a lost compare-and-set 409, in the daemon's words. */
function writable(board: MockBoard, id: string, revision: string | undefined): StoredCard {
  const card = board.cards.find((candidate) => candidate.id === id);
  if (card === undefined) {
    throw new ClientError({ status: NO_SUCH_CARD, message: `no card ${id}` });
  }
  if (closed(card)) {
    const state = card.acceptance === null ? "cancelled" : "accepted";
    throw refused(`${id} is ${state}: it takes no write; a correction is a new card, \`spec card new --corrects ${id}\`; nothing changed`);
  }
  if (revision !== undefined && revision !== card.revision) {
    throw refused(`\`${id}\` changed since this run read it: it is in ${card.column} since ${card.updated_at}; nothing changed`);
  }
  return card;
}

/** One event of one card, and the row's new `revision` and `updated_at`. */
function changed(board: MockBoard, card: StoredCard, type: string, payload: Record<string, unknown>, at: string): CardWrite {
  card.revision = String(Number(card.revision) + 1);
  card.updated_at = at;
  board.seq += 1;
  board.events.push({ id: card.id, seq: board.seq, type, at, payload: { id: card.id, ...payload } });
  return writeOf(board, card);
}

function writeOf(board: MockBoard, { id, flow, column, position, revision }: StoredCard): CardWrite {
  const card = board.cards.find((candidate) => candidate.id === id);
  return { id, flow, column, position, revision, allowed: card === undefined ? [] : allowedOf(board, card), notes: [] };
}

/** The cards standing in a column, by position. */
function standing(board: MockBoard, flow: string, column: string): StoredCard[] {
  return board.cards.filter((card) => card.flow === flow && card.column === column && card.cancellation === null).sort((a, b) => a.position - b.position);
}

/** Positions dense from 1, in the order given. */
function renumber(cards: readonly StoredCard[]): void {
  cards.forEach((card, index) => {
    card.position = index + 1;
  });
}

function nextId(board: MockBoard): string {
  const highest = board.cards.reduce((most, card) => Math.max(most, Number(card.id.slice(2))), 0);
  return `C-${String(highest + 1).padStart(4, "0")}`;
}

/** `card_new`: the first flow listing the discipline, else the one flow listing none; goes last. */
export function createCardIn(board: MockBoard, body: CardCreateBody, at: string): CardWrite {
  checkName(body.as);
  checkPerson("assignee", body.assignee);
  if (body.title.trim() === "") {
    throw refused("title: blank; a card has a title");
  }
  if (body.estimate !== undefined && !/^\d+(-\d+)?$/.test(body.estimate)) {
    throw refused(`estimate: \`${body.estimate}\` is not \`N\` or \`N-M\``);
  }
  const flow = board.flows.find((candidate) => body.discipline !== undefined && candidate.disciplines.includes(body.discipline)) ?? board.flows.find((candidate) => candidate.disciplines.length === 0);
  const column = flow?.columns[0]?.id;
  if (flow === undefined || column === undefined) {
    const asked = body.discipline === undefined ? "a card without a discipline" : `the discipline \`${body.discipline}\``;
    throw refused(`no flow takes ${asked}: none lists it, and every \`[[card.flow]]\` lists its disciplines`);
  }
  const as = body.as ?? LOCAL;
  const card: StoredCard = {
    id: nextId(board),
    flow: flow.name,
    column,
    position: standing(board, flow.name, column).length + 1,
    title: body.title,
    goal: body.goal ?? null,
    discipline: body.discipline ?? null,
    targets: body.target ?? [],
    snapshot: null,
    criteria: body.criterion ?? [],
    assignee: body.assignee ?? null,
    reviewer: null,
    estimate: body.estimate ?? null,
    due: body.due ?? null,
    parent: body.parent ?? null,
    corrects: body.corrects ?? null,
    evidence: [],
    acceptance: null,
    cancellation: null,
    created_by: as,
    created_at: at,
    updated_at: at,
    revision: "1",
  };
  board.cards.push(card);
  board.seq += 1;
  board.events.push({ id: card.id, seq: board.seq, type: "card.created", at, payload: { id: card.id, flow: card.flow, column, position: card.position, assignee: card.assignee, as } });
  return writeOf(board, card);
}

/** Why a flow refuses the move, the daemon's two lines; null when it stands. */
function moveRefusal(board: MockBoard, card: StoredCard, to: string, as: string): string | null {
  const flow = flowOf(board, card.flow);
  if (flow === undefined || to === card.column) {
    return null;
  }
  const open = allowedOf(board, card);
  const move = open.find((candidate) => candidate.to === to);
  if (move === undefined) {
    const allowed = [...open.map((candidate) => `\`${candidate.to}\``), ...(flow.accept.from === card.column ? ["`spec card accept`"] : [])];
    return `${card.id} is in ${card.column}: no move to \`${to}\`; allowed: ${allowed.length === 0 ? "nothing" : allowed.join(", ")}; nothing changed`;
  }
  if (move.by.length > 0 && !move.by.includes(as)) {
    return `${card.id} is in ${card.column}: the move to \`${to}\` is ${move.by.map((name) => `\`${name}\``).join(" or ")}'s, not \`${as}\`'s; nothing changed`;
  }
  return null;
}

/** `card_move`: leaves its column, closes it up, opens the target or goes last. */
export function moveCardIn(board: MockBoard, id: string, body: CardMoveBody, at: string): CardWrite {
  checkName(body.as);
  const card = writable(board, id, body.revision);
  const as = body.as ?? LOCAL;
  const to = body.to ?? card.column;
  const reason = moveRefusal(board, card, to, as);
  if (reason !== null) {
    throw refused(reason);
  }
  const from = card.column;
  const left = standing(board, card.flow, from).filter((other) => other !== card);
  const target = from === to ? left : standing(board, card.flow, to);
  const place = body.before === undefined ? target.length : target.findIndex((other) => other.id === body.before);
  if (place < 0) {
    throw refused(`--before: ${body.before ?? ""} is not in ${to}; nothing changed`);
  }
  card.column = to;
  renumber(left);
  renumber([...target.slice(0, place), card, ...target.slice(place)]);
  return changed(board, card, "card.moved", { from, to, position: card.position, as }, at);
}

/** A value as `edit` and `assign` take it: absent keeps `stored`, `-` clears. */
function field(written: string | undefined, stored: string | null): string | null {
  if (written === undefined) {
    return stored;
  }
  return written === CLEAR ? null : written;
}

function list(written: string[] | undefined, stored: string[]): string[] {
  if (written === undefined) {
    return stored;
  }
  return written.length === 1 && written[0] === CLEAR ? [] : written;
}

/** `card_edit`: the fields asked, evidence appended. */
export function editCardIn(board: MockBoard, id: string, body: CardEditBody, at: string): CardWrite {
  checkName(body.as);
  const card = writable(board, id, body.revision);
  const as = body.as ?? LOCAL;
  if (body.estimate !== undefined && body.estimate !== CLEAR && !/^\d+(-\d+)?$/.test(body.estimate)) {
    throw refused(`estimate: \`${body.estimate}\` is not \`N\` or \`N-M\``);
  }
  const evidence = (body.evidence ?? []).map((written) => {
    const split = written.indexOf("=");
    const [kind, value] = [written.slice(0, split), written.slice(split + 1)];
    if (kind === "url" && !/^https?:\/\//.test(value)) {
      throw refused(`evidence: \`${value}\` is no http:// or https:// url`);
    }
    return { kind, value, by: as, at };
  });
  const fields = (["title", "goal", "target", "criterion", "estimate", "due", "parent", "corrects", "evidence"] as const).filter((key) => body[key] !== undefined);
  card.title = body.title ?? card.title;
  card.goal = field(body.goal, card.goal);
  card.targets = list(body.target, card.targets);
  card.criteria = list(body.criterion, card.criteria);
  card.estimate = field(body.estimate, card.estimate);
  card.due = field(body.due, card.due);
  card.parent = field(body.parent, card.parent);
  card.corrects = field(body.corrects, card.corrects);
  card.evidence = [...card.evidence, ...evidence];
  return changed(board, card, "card.edited", { fields, as }, at);
}

/** `card_assign`: `local` and `agent` are no one's name, so neither can review. */
export function assignCardIn(board: MockBoard, id: string, body: CardAssignBody, at: string): CardWrite {
  checkName(body.as);
  checkPerson("assignee", body.assignee);
  checkPerson("reviewer", body.reviewer);
  const card = writable(board, id, body.revision);
  if (body.reviewer === LOCAL || body.reviewer === "agent") {
    throw refused(`reviewer: \`${body.reviewer}\` can be no reviewer: a person accepts a result`);
  }
  card.assignee = field(body.assignee, card.assignee);
  card.reviewer = field(body.reviewer, card.reviewer);
  return changed(board, card, "card.assigned", { assignee: card.assignee, reviewer: card.reviewer, as: body.as ?? LOCAL }, at);
}

/** `card_cancel`: out of its column at `position` 0, the column closing up. */
export function cancelCardIn(board: MockBoard, id: string, body: CardCancelBody, at: string): CardWrite {
  checkName(body.as);
  const card = writable(board, id, body.revision);
  const as = body.as ?? LOCAL;
  card.cancellation = { by: as, at, reason: body.reason };
  card.position = 0;
  renumber(standing(board, card.flow, card.column));
  return changed(board, card, "card.cancelled", { column: card.column, as }, at);
}

/** `card_accept`: in `accept.from`, the name its reviewer, never its assignee; the rest of the core's order. */
export function acceptCardIn(board: MockBoard, id: string, body: CardAcceptBody, at: string): CardWrite {
  checkName(body.as);
  const card = writable(board, id, undefined);
  const flow = flowOf(board, card.flow);
  if (flow === undefined || card.column !== flow.accept.from) {
    throw refused(`${id} is in ${card.column}: \`accept\` needs ${flow?.accept.from ?? ""}; nothing changed`);
  }
  if (card.assignee === body.as) {
    throw refused(`${id}: \`${body.as}\` is its assignee, who never accepts a result; nothing changed`);
  }
  if (card.reviewer === null) {
    throw refused(`${id} has no reviewer: \`spec card assign ${id} --reviewer N\` names one; nothing changed`);
  }
  if (card.reviewer !== body.as) {
    throw refused(`${id}: its reviewer is \`${card.reviewer}\`, and \`--as\` says \`${body.as}\`; nothing changed`);
  }
  writable(board, id, body.revision);
  const from = card.column;
  const to = flow.accept.to;
  const left = standing(board, card.flow, from).filter((other) => other !== card);
  card.column = to;
  card.acceptance = { by: body.as, at, note: body.note };
  renumber(left);
  card.position = standing(board, card.flow, to).length;
  return changed(board, card, "card.accepted", { from, to, position: card.position, as: body.as }, at);
}

/** The `conflict` scenario: someone else got there first, once. */
export function changeElsewhere(board: MockBoard, id: string, at: string): void {
  const card = board.cards.find((candidate) => candidate.id === id);
  if (card !== undefined && !closed(card)) {
    changed(board, card, "card.edited", { fields: ["goal"], as: ELSEWHERE }, at);
  }
}
