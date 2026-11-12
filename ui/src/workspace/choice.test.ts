import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { aModule, aWorkspace, aWorkspaces, anImplicitWorkspaces } from "../test/builders";
import { ALL_MODULES, choosesByLogin, landingHash, moduleKey, moduleOf, storedWorkspace, useWorkspaceChoice, workspaceKey, workspaceOf } from "./choice";

// docs/features/workspaces.md "The UI", its choice: the two keys per project, a stored name
// looked up among the served ones, the landing's hash, and a refused write held until a reload
// (WS-3's M5 and M6 at the unit's level; the screen's: src/mocks/workspaces.smoke.test.tsx).

const DOCUMENT = aWorkspaces(
  [aWorkspace({ name: "w-one", title: "One", landing: "board", sections: ["board", "tree"], module: "m-two" }), aWorkspace({ name: "w-two", title: "Two" })],
  [aModule({ name: "m-one", root: "ROOT-ONE" }), aModule({ name: "m-two", root: "ROOT-TWO" })],
);

describe("the keys", () => {
  it("are per project: the routed slug after a fixed prefix (M6)", () => {
    expect(workspaceKey("a b", null)).toBe("specengine.workspace.a b");
    expect(moduleKey("a b", null)).toBe("specengine.module.a b");
    expect(ALL_MODULES).toBe("");
  });
});

describe("the chosen workspace", () => {
  it("is the stored name while served, else the first", () => {
    expect(workspaceOf(DOCUMENT, "w-two")?.title).toBe("Two");
    expect(workspaceOf(DOCUMENT, null)?.title).toBe("One");
    expect(workspaceOf(DOCUMENT, "gone")?.title).toBe("One");
    expect(workspaceOf({ ...DOCUMENT, workspaces: [] }, null)).toBeNull();
  });
});

describe("the chosen module", () => {
  const one = workspaceOf(DOCUMENT, null);
  const two = workspaceOf(DOCUMENT, "w-two");

  it("is none for All modules, a served name's, else the workspace's own", () => {
    expect(moduleOf(DOCUMENT, one, ALL_MODULES)).toBeNull();
    expect(moduleOf(DOCUMENT, one, "m-one")?.root).toBe("ROOT-ONE");
    expect(moduleOf(DOCUMENT, one, null)?.root).toBe("ROOT-TWO");
    expect(moduleOf(DOCUMENT, two, null)).toBeNull();
    // A stored name no longer served: as no key.
    expect(moduleOf(DOCUMENT, one, "gone")?.root).toBe("ROOT-TWO");
    expect(moduleOf(DOCUMENT, two, "gone")).toBeNull();
    // A workspace's module the document does not serve: none (the reader nulls it with a note).
    expect(moduleOf(DOCUMENT, aWorkspace({ name: "w-three", module: "m-gone" }), null)).toBeNull();
  });
});

describe("the landing", () => {
  it("is the landing's section, or the home for the Overview and for a value the UI does not know", () => {
    expect(landingHash("a b", workspaceOf(DOCUMENT, null))).toBe("#/a%20b/board");
    expect(landingHash("p", aWorkspace({ name: "w", landing: "overview" }))).toBe("#/p");
    expect(landingHash("p", aWorkspace({ name: "w", landing: "stage" }))).toBe("#/p");
    expect(landingHash("p", null)).toBe("#/p");
  });
});

describe("useWorkspaceChoice", () => {
  it("reads nothing without a document", () => {
    const { result } = renderHook(() => useWorkspaceChoice("p", null, undefined));
    expect([result.current.workspace, result.current.module]).toEqual([null, null]);
  });

  it("stores a workspace's name and removes the module key; a module's name, or All modules as the empty string", () => {
    window.localStorage.setItem(moduleKey("p", null), "m-one");
    const { result } = renderHook(() => useWorkspaceChoice("p", null, DOCUMENT));
    expect([result.current.workspace?.name, result.current.module?.name]).toEqual(["w-one", "m-one"]);
    act(() => {
      result.current.chooseWorkspace("w-two");
    });
    expect(window.localStorage.getItem(workspaceKey("p", null))).toBe("w-two");
    expect(window.localStorage.getItem(moduleKey("p", null))).toBeNull();
    expect([result.current.workspace?.name, result.current.module]).toEqual(["w-two", null]);
    act(() => {
      result.current.chooseModule("m-two");
    });
    expect(window.localStorage.getItem(moduleKey("p", null))).toBe("m-two");
    expect(result.current.module?.root).toBe("ROOT-TWO");
    act(() => {
      result.current.chooseModule(ALL_MODULES);
    });
    expect(window.localStorage.getItem(moduleKey("p", null))).toBe("");
    expect(result.current.module).toBeNull();
    // Another project's keys are its own.
    expect(window.localStorage.getItem(workspaceKey("q", null))).toBeNull();
  });

  it("holds a choice the browser refuses to store until a reload: a new hook reads the storage again (M5)", () => {
    const refused = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new DOMException("The quota has been exceeded.", "QuotaExceededError");
    });
    const first = renderHook(() => useWorkspaceChoice("p", null, DOCUMENT));
    act(() => {
      first.result.current.chooseWorkspace("w-two");
    });
    expect(refused).toHaveBeenCalled();
    expect(first.result.current.workspace?.name).toBe("w-two");
    act(() => {
      first.result.current.chooseModule("m-one");
    });
    expect(first.result.current.module?.name).toBe("m-one");
    first.unmount();
    const second = renderHook(() => useWorkspaceChoice("p", null, DOCUMENT));
    expect([second.result.current.workspace?.name, second.result.current.module?.name]).toEqual(["w-one", "m-two"]);
  });

  it("reads storage that throws as no choice", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new DOMException("The operation is insecure.", "SecurityError");
    });
    const { result } = renderHook(() => useWorkspaceChoice("p", null, DOCUMENT));
    expect([result.current.workspace?.name, result.current.module?.name]).toEqual(["w-one", "m-two"]);
  });

  it("serves the implicit workspace and no module for a project that configures none", () => {
    const { result } = renderHook(() => useWorkspaceChoice("p", null, anImplicitWorkspaces()));
    expect(result.current.workspace?.sections).toEqual(["inbox", "board", "roadmap", "flow", "tree", "graph", "health", "questions"]);
    expect(result.current.module).toBeNull();
  });
});

// docs/features/hub-department.md "Acceptance criteria", HD3 at the unit's level: the keys per login on
// a hub, a stored choice winning over the served default, and no key read or written while no login is
// known (the identity read failed); the screen's over the mock: src/mocks/hubDepartment.smoke.test.tsx.
describe("a hub login's department (HD3)", () => {
  const SERVED = { ...DOCUMENT, default: "w-two" };

  it("keys a hub's choice per login, a local one as before", () => {
    expect([workspaceKey("p", "mira"), moduleKey("p", "mira")]).toEqual(["specengine.workspace.p.mira", "specengine.module.p.mira"]);
    expect([workspaceKey("p", null), moduleKey("p", null)]).toEqual(["specengine.workspace.p", "specengine.module.p"]);
  });

  it("chooses a stored served name, else the default while served, else the first", () => {
    expect(workspaceOf(SERVED, "w-one")?.name).toBe("w-one");
    expect(workspaceOf(SERVED, null)?.name).toBe("w-two");
    expect(workspaceOf(SERVED, "gone")?.name).toBe("w-two");
    expect(workspaceOf({ ...DOCUMENT, default: "gone" }, null)?.name).toBe("w-one");
  });

  it("reads and writes the login's keys alone: the browser-wide key stays as it was", () => {
    // A third served name in the browser-wide key, neither the default nor the choice below: a read of
    // it lands there instead of the default, a write to it too changes it (M: both keys written).
    const three = { ...SERVED, workspaces: [...SERVED.workspaces, aWorkspace({ name: "w-three", title: "Three" })] };
    window.localStorage.setItem(workspaceKey("p", null), "w-three");
    window.localStorage.setItem(moduleKey("p", "mira"), "m-one");
    const { result } = renderHook(() => useWorkspaceChoice("p", "mira", three));
    expect([result.current.workspace?.name, result.current.module?.name]).toEqual(["w-two", "m-one"]);
    act(() => {
      result.current.chooseWorkspace("w-one");
    });
    expect([window.localStorage.getItem(workspaceKey("p", "mira")), window.localStorage.getItem(moduleKey("p", "mira")), window.localStorage.getItem(workspaceKey("p", null))]).toEqual([
      "w-one",
      null,
      "w-three",
    ]);
    expect(storedWorkspace("p", "mira", three)?.name).toBe("w-one");
    expect(storedWorkspace("p", "ann", three)?.name).toBe("w-two");
    expect(storedWorkspace("p", null, three)?.name).toBe("w-three");
  });

  it("with no login known reads no key, lands on the default, and holds a choice without writing it", () => {
    window.localStorage.setItem(workspaceKey("p", null), "w-one");
    const setItem = vi.spyOn(Storage.prototype, "setItem");
    const removeItem = vi.spyOn(Storage.prototype, "removeItem");
    const { result } = renderHook(() => useWorkspaceChoice("p", undefined, SERVED));
    expect(result.current.workspace?.name).toBe("w-two");
    expect(storedWorkspace("p", undefined, SERVED)?.name).toBe("w-two");
    act(() => {
      result.current.chooseWorkspace("w-one");
    });
    act(() => {
      result.current.chooseModule("m-one");
    });
    expect([result.current.workspace?.name, result.current.module?.name]).toEqual(["w-one", "m-one"]);
    expect([setItem.mock.calls.length, removeItem.mock.calls.length]).toEqual([0, 0]);
  });

  it("names a lens the login can change: more than one department, or a module", () => {
    expect(choosesByLogin(DOCUMENT)).toBe(true);
    expect(choosesByLogin(anImplicitWorkspaces())).toBe(false);
    expect(choosesByLogin(aWorkspaces([aWorkspace({ name: "w-solo" })], [aModule({ name: "m", root: "R" })]))).toBe(true);
  });
});
