import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { aModule, aWorkspace, aWorkspaces, anImplicitWorkspaces } from "../test/builders";
import { ALL_MODULES, landingHash, moduleKey, moduleOf, useWorkspaceChoice, workspaceKey, workspaceOf } from "./choice";

// docs/features/workspaces.md "The UI", its choice: the two keys per project, a stored name
// looked up among the served ones, the landing's hash, and a refused write held until a reload
// (WS-3's M5 and M6 at the unit's level; the screen's: src/mocks/workspaces.smoke.test.tsx).

const DOCUMENT = aWorkspaces(
  [aWorkspace({ name: "w-one", title: "One", landing: "board", sections: ["board", "tree"], module: "m-two" }), aWorkspace({ name: "w-two", title: "Two" })],
  [aModule({ name: "m-one", root: "ROOT-ONE" }), aModule({ name: "m-two", root: "ROOT-TWO" })],
);

describe("the keys", () => {
  it("are per project: the routed slug after a fixed prefix (M6)", () => {
    expect(workspaceKey("a b")).toBe("specengine.workspace.a b");
    expect(moduleKey("a b")).toBe("specengine.module.a b");
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
    const { result } = renderHook(() => useWorkspaceChoice("p", undefined));
    expect([result.current.workspace, result.current.module]).toEqual([null, null]);
  });

  it("stores a workspace's name and removes the module key; a module's name, or All modules as the empty string", () => {
    window.localStorage.setItem(moduleKey("p"), "m-one");
    const { result } = renderHook(() => useWorkspaceChoice("p", DOCUMENT));
    expect([result.current.workspace?.name, result.current.module?.name]).toEqual(["w-one", "m-one"]);
    act(() => {
      result.current.chooseWorkspace("w-two");
    });
    expect(window.localStorage.getItem(workspaceKey("p"))).toBe("w-two");
    expect(window.localStorage.getItem(moduleKey("p"))).toBeNull();
    expect([result.current.workspace?.name, result.current.module]).toEqual(["w-two", null]);
    act(() => {
      result.current.chooseModule("m-two");
    });
    expect(window.localStorage.getItem(moduleKey("p"))).toBe("m-two");
    expect(result.current.module?.root).toBe("ROOT-TWO");
    act(() => {
      result.current.chooseModule(ALL_MODULES);
    });
    expect(window.localStorage.getItem(moduleKey("p"))).toBe("");
    expect(result.current.module).toBeNull();
    // Another project's keys are its own.
    expect(window.localStorage.getItem(workspaceKey("q"))).toBeNull();
  });

  it("holds a choice the browser refuses to store until a reload: a new hook reads the storage again (M5)", () => {
    const refused = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new DOMException("The quota has been exceeded.", "QuotaExceededError");
    });
    const first = renderHook(() => useWorkspaceChoice("p", DOCUMENT));
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
    const second = renderHook(() => useWorkspaceChoice("p", DOCUMENT));
    expect([second.result.current.workspace?.name, second.result.current.module?.name]).toEqual(["w-one", "m-two"]);
  });

  it("reads storage that throws as no choice", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new DOMException("The operation is insecure.", "SecurityError");
    });
    const { result } = renderHook(() => useWorkspaceChoice("p", DOCUMENT));
    expect([result.current.workspace?.name, result.current.module?.name]).toEqual(["w-one", "m-two"]);
  });

  it("serves the implicit workspace and no module for a project that configures none", () => {
    const { result } = renderHook(() => useWorkspaceChoice("p", anImplicitWorkspaces()));
    expect(result.current.workspace?.sections).toEqual(["inbox", "board", "roadmap", "flow", "tree", "graph", "health", "questions"]);
    expect(result.current.module).toBeNull();
  });
});
