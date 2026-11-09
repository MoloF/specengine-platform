---
class: canon
tier: 2
scope: [ui]
owner: owner
reviewed: 2026-11-09
---

# The Flow screen: the place in the hash

Where Flow stands, held in the URL, so a reload, a shared link, Back and Forward draw one picture whatever the viewer stored; `writer-studio`, code `ui/src/app/routes.ts` (`flowHash`, `parseFlowPlace`), `ui/src/flow/FlowView.tsx`. The screen: `spec-flow-screen.md`.

## The place

```text
#/<p>/flow[/<name>][?<key>=<value>[&<key>=<value>]...]
module=<m>                         a served module's name; empty: All modules
scope=<ref>                        the read's root: a group's name, a module's root
stage=<s>                          a served lane's stage; `-`: No stage
edge=<from>,<to>,<type>[,<kind>]   no fourth part: kind null
```

`FlowPlace {module, scope, stage, select}`: `module` null when absent (the stored choice), `""` All modules; `scope` null for the lens root (`spec-flow-screen.md` "Reads"); `stage` null for All stages; `select` `{node}`, `{edge: {from, to, type, kind}}` or null. The `/<name>` segment selects a node, as `sectionHash` writes it.

- **Splitting.** `parseHash` splits off the first raw `?` for `flow` alone; a `?` in a Tree id stays in it.
- **Writing.** `flowHash(project, place)`: keys in the order module, scope, stage, edge, each only when non-null; each value and each edge part through `encodeURIComponent`, the parts joined by a raw `,`.
- **Reading.** `parseFlowPlace(route)` decodes with `decodeURIComponent`, never `URLSearchParams` (`+` is no space). A key drops when its value fails to decode, when it is unknown, when `scope` or `stage` is empty, or when `edge` has fewer than three parts or an empty one. The first of a repeated key wins; a name wins over an `edge`.
- **Written hashes** name the effective module whenever `[[module]]` serves one (`module=` for All), never otherwise. A literal stage `-` cannot be asked, as in `spec trace`.
- Every place link is an `<a>` whose `href` is a `flowHash` (`policy.test.ts`).

Examples: `#/harbor-sim/flow/MEC-MOORING?module=&scope=DOM-BERTHS&stage=quayside`; `#/harbor-sim/flow?module=&edge=MEC-FAIRWAY,DOM-BERTHS,follows,alternative`, a lifted edge; `#/harbor-sim/flow/MEC-MOORING`, an old link, which seeks.

## History

A scope, stage or module change pushes: a place link, Space, a double-click, Open. A selection, an arrow, Esc's clear, a seek's fix and a correction replace; a gap activation's seek pushes. On entering Flow, a bare `#/<p>/flow` (the nav's link) is replaced by the project's last place in this Shell's life; within Flow a bare hash is the lens root.

## The module

The hash's `module` wins; with none, `specengine.module.<p>` (`workspaces.md`) is the default, written only by the module links of the places pane, which push. A module no `[[module]]` serves, or a stage at an unrooted Top, drops by a replace. With a module, its `root` is the lens root and Top is not read.

## The seek

A selected name the drawing read lacks seeks once per place: `getTree({})`, then a replace with `{module, scope: parent, stage: null, select}`; a parent equal to the lens root gives `scope: null`, a top document `module: ""`. A lens root not yet known reads Top first (`useTopOnDemand`), so no lens root is written as a scope. A name the tree lacks: a note and a Tree link. The scope's own node selected is the scope view: its selection drops by a replace. An arrow not drawn drops by a replace, announcing `flow.edgeGone`.

## Memory

`FlowState`, per project in the Shell for the page's life: `{tab: "canvas" | "list" | null, place}`. The hash is the store; `place` stands in for a bare hash on entry alone. No `localStorage` key of Flow's own.

## Places

The left pane (`FlowPlaces.tsx`), a `<nav>` "Flow places" apart from the app's; every item an `<a>`, so a click pushes.

- **Modules**, when `[[module]]` serves one: All modules, then titles, the one in force `aria-current`.
- **Scope**: Up, then the scope's title (`aria-current="location"`). Up names the holder a held read gives (`useHeldHolder`), else the lens root; none at the lens root; a lens root not yet read: `flow.upStart`.
- **Stages**, from the summary: All stages, the lanes in served order, each `<c> of <n> confirmed` and a bar from served `counts`, No stage last. At an unrooted Top: `flow.stagesAtTop`, no read. A failed summary: `flow.stagesFailed` and Retry; the canvas stands.
- **Under the current item**: for All stages, the scope's groups (`holds > 0`); for a stage, the distinct `group`s its nodes name, first seen, the scope left out.

## Known limits

- **Up from held reads**: a rooted read leaves its root's `group` null. A fresh deep mount says `flow.upStart`, going to the lens root, maybe skipping a level; a held holder is named by its ID, not its title.
