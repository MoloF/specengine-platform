import { useId, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { apiErrorOf } from "../api/client";
import { useCard, useCards, useCardWrite, useCardWriting, useTrace } from "../api/queries";
import type { Card, CardFlow, CardMoveBody, CardRow, CardWrite } from "../api/types";
import { replaceHash } from "../app/location";
import { cardPageHash, sectionHash } from "../app/routes";
import { useOpenShortcuts } from "../app/shortcuts";
import { useTranslate } from "../i18n";
import { useAnnounce } from "../ui/announcer";
import { buttonClasses } from "../ui/Button";
import { Icon } from "../ui/Icon";
import { hasModifier, isTextField } from "../ui/keys";
import { shownSlot } from "../ui/text";
import { useFocusLater } from "../ui/useFocusLater";
import { actingName, useActing, useAllowed } from "./acting";
import { CardActDialog, moveChoicesOf, type BoardChoice, type OpenDialog, type Said } from "./CardActs";
import { CardView, isCard, titleOf, type CardAction, type CardLinks } from "./CardPanel";
import { useToday } from "./due";
import { openRows, reorderOf } from "./filter";
import { columnTitle, rowMark } from "./labels";
import { MoveMenu } from "./MoveMenu";

/** A refused move: the daemon's reason; `count` a new alert for each. */
interface Refusal {
  message: string;
  count: number;
}

/**
 * A card's page (`#/<p>/board/<C>/page`, `ui/README.md` "Screen rules", Shell): no columns, the work
 * item the modal shows (CardPanel.tsx `CardView`), its title the one `h1`, "Open on the board" on the
 * breadcrumb line. It reads what the board reads, the list (for the flows and the mark) and the
 * card, so a reload keeps it; the work item reads the card's children itself; the nav marks Board. A link to another card goes to that card's page, replacing this one.
 * Nothing here judges a column: the moves are the card's `allowed`, the daemon answers.
 */
export function CardPage({ project, id }: { project: string; id: string }) {
  const t = useTranslate();
  const titleId = useId();
  const announce = useAnnounce();
  const focusLater = useFocusLater();
  const openShortcuts = useOpenShortcuts();
  const cards = useCards(project);
  const query = useCard(project, id);
  const trace = useTrace(project, {}, true, true);
  const write = useCardWrite(project);
  const writing = useCardWriting(project);
  const today = useToday();
  const allows = useAllowed(project);
  const [acting] = useActing();
  const [menu, setMenu] = useState<{ anchor: DOMRect; trigger: HTMLElement } | null>(null);
  const [dialog, setDialog] = useState<OpenDialog | null>(null);
  const [refusal, setRefusal] = useState<Refusal | null>(null);
  const [writeNotes, setWriteNotes] = useState<readonly string[]>([]);
  const heading = useRef<HTMLHeadingElement>(null);
  const moving = useRef(false);
  const refusals = useRef(0);

  const list = cards.data;
  const all = useMemo(() => list?.cards ?? [], [list]);
  const rollups = useMemo(() => new Map(trace.error === null ? (trace.data?.nodes.map((node) => [node.id, node.rollup]) ?? []) : []), [trace.data, trace.error]);
  const row = all.find((candidate) => candidate.id === id);
  const card = query.data !== undefined && isCard(query.data) ? query.data : undefined;
  const title = titleOf(query);

  function flowOf(name: string): CardFlow | undefined {
    return list?.flows.find((candidate) => candidate.name === name);
  }

  const links: CardLinks = {
    project,
    page: true,
    onFollow: (other) => {
      replaceHash(cardPageHash(project, other));
      focusLater(() => heading.current);
    },
  };

  /** The menu's lines: the card's `allowed`, Return…, Up and Down in its column of the list, Accept. */
  function choices(): BoardChoice[] {
    if (card === undefined) {
      return [];
    }
    const flow = flowOf(card.flow);
    const column = all.filter((candidate) => candidate.flow === card.flow && candidate.column === card.column);
    const standing = openRows(column);
    const reorder = row === undefined ? null : { ...reorderOf(id, standing, standing), column: card.column };
    return moveChoicesOf({ row: card, flow, moves: card.allowed, reorder, lane: columnTitle(flow, card.column), accepts: allows("accept"), t });
  }

  function closeMenu() {
    menu?.trigger.focus();
    setMenu(null);
  }

  async function move(target: Pick<CardRow, "id" | "revision" | "flow" | "column">, asked: Omit<CardMoveBody, "revision" | "as">, spoken: string | null) {
    if (moving.current) {
      return;
    }
    moving.current = true;
    setRefusal(null);
    try {
      const answer = await write.mutateAsync({ write: "move", id: target.id, body: { ...asked, revision: target.revision, as: actingName(acting) } });
      setWriteNotes(answer.notes);
      announce(spoken ?? t("board.said.moved", { id: target.id, column: columnTitle(flowOf(answer.flow ?? target.flow), answer.column ?? target.column) }));
    } catch (error) {
      refusals.current += 1;
      setRefusal({ message: apiErrorOf(error).message, count: refusals.current });
    } finally {
      moving.current = false;
    }
  }

  function takeChoice(choice: BoardChoice, trigger: HTMLElement) {
    closeMenu();
    if (card === undefined) {
      return;
    }
    if (choice.act === "accept") {
      setDialog({ action: "accept", card, trigger });
    } else if (choice.act === "return") {
      setDialog({ action: "return", card, to: choice.asked.to ?? card.column, before: null, trigger });
    } else {
      void move(card, choice.asked, choice.spoken);
    }
  }

  function onAct(action: CardAction, acted: Card, trigger: HTMLElement) {
    if (writing) {
      return;
    }
    if (action === "move") {
      setMenu({ anchor: trigger.getBoundingClientRect(), trigger });
    } else if (action === "return") {
      const back = flowOf(acted.flow)?.return ?? null;
      if (back !== null) {
        setDialog({ action: "return", card: acted, to: back, before: null, trigger });
      }
    } else {
      setDialog({ action, card: acted, trigger });
    }
  }

  /** The card a dialog works on, as read now: its revision is what a submit goes on. */
  function currentCard(asked: Card): Card {
    if (card === undefined || card.id !== asked.id) {
      return asked;
    }
    return card;
  }

  function closeDialog() {
    const back = dialog?.trigger;
    setDialog(null);
    focusLater(() => (back?.isConnected === true ? back : heading.current));
  }

  function written(answer: CardWrite, said: Said) {
    setRefusal(null);
    setWriteNotes(answer.notes);
    closeDialog();
    announce(said(answer.id ?? t("board.said.theCard")));
  }

  function onKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.defaultPrevented || hasModifier(event) || isTextField(event.target) || dialog !== null || menu !== null) {
      return;
    }
    if (event.key === "?") {
      event.preventDefault();
      openShortcuts();
    }
  }

  return (
    <section className="view card-page" aria-labelledby={titleId} onKeyDown={onKeyDown}>
      <CardView
        heading={
          <h1 ref={heading} id={titleId} tabIndex={-1}>
            <bdi>{title === null ? id : shownSlot(title)}</bdi>
          </h1>
        }
        tools={
          <a className={buttonClasses("subtle", "compact")} href={sectionHash(project, "board", id)}>
            <Icon name="arrowLeft" />
            <span>{t("board.page.openOnBoard")}</span>
          </a>
        }
        notices={
          <>
            {refusal !== null && (
              <div key={refusal.count} className="refusal" role="alert">
                <p className="refusal-title">
                  <Icon name="alert" />
                  <span>{t("board.view.refused")}</span>
                </p>
                <p className="verbatim" lang="en">
                  {refusal.message}
                </p>
              </div>
            )}
            <div role="status">
              {writeNotes.length > 0 && (
                <ul className="note-list" aria-label={t("board.view.writeNotes")}>
                  {writeNotes.map((note, index) => (
                    <li key={`${String(index)}-${note}`} className="verbatim-note" lang="en">
                      {note}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </>
        }
        project={project}
        id={id}
        query={query}
        flowOf={flowOf}
        today={today}
        mark={cards.error !== null || row === undefined ? null : rowMark(row)}
        rollups={rollups}
        markFailure={cards.error === null ? null : apiErrorOf(cards.error).message}
        stateFailure={trace.error === null ? null : apiErrorOf(trace.error).message}
        links={links}
        onAct={onAct}
      />
      {menu !== null && card !== undefined && (
        <MoveMenu
          label={t("board.menu.label", { id })}
          anchor={menu.anchor}
          choices={choices()}
          empty={t("board.menu.empty")}
          onChoose={(choice) => {
            takeChoice(choice, menu.trigger);
          }}
          onClose={closeMenu}
        />
      )}
      {dialog !== null && (
        <CardActDialog
          project={project}
          dialog={dialog}
          flowOf={flowOf}
          flows={list?.flows ?? []}
          current={{ row: (other) => all.find((candidate) => candidate.id === other), card: currentCard }}
          clearOn={row === undefined ? null : (rowMark(row)?.on ?? null)}
          t={t}
          onClose={closeDialog}
          onActingLine={() => undefined}
          onWritten={(answer, said) => {
            written(answer, said);
          }}
          onRaised={(said) => {
            setRefusal(null);
            closeDialog();
            announce(said);
          }}
        />
      )}
    </section>
  );
}
