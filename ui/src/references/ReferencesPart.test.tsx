import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { ClientError, type UploadProgress } from "../api/client";
import type { BlobStored, Card, QueueEvent, Reference } from "../api/types";
import { switchLanguage } from "../i18n";
import { ru } from "../i18n/ru";
import { aB3, aBlobStored, aCard, aCardList, aCardRow, aLinkReference, aNode, aProposal, aReference, aTaskEntry, aTaskPackage, entryOf } from "../test/builders";
import { fileOf, pngBytes } from "../test/images";
import { renderApp } from "../test/render";
import { argsOf, stubClient, type StubClient } from "../test/stubClient";

// docs/features/work-references.md "UI" and RF21, over the stub board: the References part in a task's
// modal above its Comments, in the document view and in the proposal pane. Tiles newest first: a raster
// image as an `<img loading="lazy">` of its agent copy (else its file) opening the file in a dialog;
// audio `<audio controls preload="none">`; video `<video controls preload="metadata">`; a model, a PDF,
// a `.blend` (with its warning) and anything not a raster image served inline a download link; a link
// an anchor with `rel="noopener noreferrer nofollow"`; a lost file and a tombstone said in words;
// people's text as text. Add a file…: a 4000 px image's copy uploaded before it and named in the
// attach; progress from `onProgress`, Cancel aborting; a 400 keeping the dialog and the uploaded file;
// a 404 naming a file gone uploading it once more; a pick past 256 MiB refused unsent. Add a link….
// Delete through a dialog, off with `ActOff` where `allowed` lacks `delete_reference`. A
// `reference.*` event reads the target again; the part in Russian.

beforeAll(async () => {
  // The part's chunk and the work item's, the document's and the markdown's, loaded once, so a test meets them drawn.
  await import("./ReferencesPart");
  await import("../board/CardItems");
  await import("../markdown/Markdown");
  await import("../doc/DocBody");
});

afterEach(() => {
  vi.unstubAllGlobals();
});

const MIB = 1_048_576;
const CARD: Card = aCard({ id: "C-0007", title: "Rail height", targets: ["MEC-RAIL"] });

/** A file reference of a type, its b3 `seed`'s. */
function file(id: string, seed: string, media: string, cls: string, fields: Partial<Reference> = {}): Reference {
  return aReference({ id, b3: aB3(seed), media, class: cls, meta: { width: null, height: null, duration_ms: null, triangles: null }, ...fields });
}

/** Every kind of tile, oldest first. */
function tiles(): Reference[] {
  return [
    aLinkReference({ id: "RF-0010", matters: "The palette, not the layout." }),
    aReference({
      id: "RF-0011",
      title: "Harbour at dusk",
      b3: aB3("a"),
      agent_b3: aB3("c"),
      size: 2_202_009,
      meta: { width: 4032, height: 3024, duration_ms: null, triangles: null },
      matters: "The light on the water, <b>not</b> the boats",
    }),
    file("RF-0012", "d", "audio/wav", "audio", { title: "Foghorn", name: "foghorn.wav", matters: "Two long blasts.", meta: { width: null, height: null, duration_ms: 2000, triangles: null } }),
    file("RF-0013", "e", "video/mp4", "video", { title: "Docking", name: "docking.mp4", matters: "The approach angle." }),
    file("RF-0014", "1", "model/gltf-binary", "model", { title: "Crane", name: "crane.glb", inline: false, matters: "Its reach.", meta: { width: null, height: null, duration_ms: null, triangles: 16 } }),
    file("RF-0015", "2", "application/x-blender", "model", { title: "Quay", name: "quay.blend", inline: false, matters: "The bollards.", warning: "a .blend is only downloaded here; Blender can run the scripts it holds: open it with Auto Run Python Scripts off" }),
    file("RF-0016", "3", "application/pdf", "pdf", { title: "Tide table", name: "tides.pdf", inline: false, matters: "The spring tides." }),
    aReference({ id: "RF-0017", title: "Old pier", file: "lost" }),
    aReference({ id: "RF-0018", actor: "lev", author: { type: "human", role: null, model: null, run: null, login: "lev" }, title: null, name: null, b3: null, meta: null, file: null, deleted: { by: "lev", at: "2026-11-23T11:00:00Z" } }),
    file("RF-0019", "4", "image/svg+xml", "image", { title: "Vector map", name: "map.svg", inline: true }),
  ];
}

function client(references: Reference[] = tiles()): StubClient {
  const stub = stubClient();
  stub.getCards.mockImplementation(() => Promise.resolve(aCardList([aCardRow({ id: CARD.id, title: CARD.title, targets: CARD.targets })])));
  stub.getCard.mockImplementation((_project, id) => Promise.resolve(id === CARD.id ? structuredClone(CARD) : { id, reason: `no card ${id}` }));
  stub.state.references.push(...references);
  return stub;
}

/** A hub page as `login`, its `[team]` row allowing `allowed`. */
function onHub(stub: StubClient, login: string, allowed: string[]): StubClient {
  stub.getIdentity.mockImplementation(() => Promise.resolve({ login }));
  stub.getTeam.mockImplementation(() => Promise.resolve({ login, name: null, allowed }));
  return stub;
}

/** The task's modal and its References part, once read: the part's own region, never its lazy fallback's, which the part replaces. */
async function part(stub: StubClient): Promise<HTMLElement> {
  renderApp(stub, `#/alpha/board/${CARD.id}`);
  const dialog = await screen.findByRole("dialog", { name: CARD.title });
  const heading = await within(dialog).findByRole("heading", { level: 3, name: /^References · / });
  const region = heading.closest("section");
  if (region === null) {
    throw new Error("no References region");
  }
  return region;
}

/** One tile's article by its reference's ID. */
function tile(scope: HTMLElement, id: string): HTMLElement {
  const found = scope.querySelector<HTMLElement>(`[data-reference="${id}"]`);
  if (found === null) {
    throw new Error(`no ${id}`);
  }
  return found;
}

/** A tile's Delete…. */
function deleteOf(article: HTMLElement): HTMLElement {
  return within(article).getByRole("button", { name: "Delete…" });
}

describe("the tiles (RF21)", () => {
  it("stands above the Comments part, lists newest first, the count in its heading", async () => {
    const shown = await part(client());
    expect(within(shown).getByRole("heading", { level: 3 }).textContent).toBe("References · 10");
    expect(Array.from(shown.querySelectorAll("[data-reference]"), (article) => article.getAttribute("data-reference"))).toEqual([
      "RF-0019",
      "RF-0018",
      "RF-0017",
      "RF-0016",
      "RF-0015",
      "RF-0014",
      "RF-0013",
      "RF-0012",
      "RF-0011",
      "RF-0010",
    ]);
    const comments = shown.parentElement?.querySelector("section.comments[aria-label='Comments']");
    if (comments === null || comments === undefined) {
      throw new Error("no Comments part beside it");
    }
    expect(shown.compareDocumentPosition(comments) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("draws a raster image from its agent copy, lazily, and opens the file itself in a dialog (M: the original's bytes shown as the copy)", async () => {
    const stub = client();
    const shown = await part(stub);
    const image = tile(shown, "RF-0011");
    const img = image.querySelector("img");
    expect([img?.getAttribute("src"), img?.getAttribute("loading"), img?.getAttribute("alt")]).toEqual([`/api/projects/alpha/blobs/${aB3("c")}`, "lazy", "Harbour at dusk"]);
    expect(image.textContent).toContain("4032×3024");
    expect(image.textContent).toContain("2.1 MiB");
    expect(image.textContent).toContain("agent copy");
    // Without a copy, the file itself.
    expect(tile(shown, "RF-0017").querySelector("img")).toBeNull();
    fireEvent.click(within(image).getByRole("button", { name: "Open Harbour at dusk" }));
    const opened = await screen.findByRole("dialog", { name: "Harbour at dusk" });
    expect(opened.querySelector("img")?.getAttribute("src")).toBe(`/api/projects/alpha/blobs/${aB3("a")}`);
    expect(within(opened).getByRole("link", { name: "Download" }).getAttribute("href")).toBe(`/api/projects/alpha/blobs/${aB3("a")}`);
  });

  it("previews inline only the raster types served inline: an SVG and an image served as an attachment are download links (M: a non-raster previewed inline)", async () => {
    const shown = await part(client([...tiles(), file("RF-0020", "5", "image/png", "image", { title: "Attached PNG", name: "plan.png", inline: false })]));
    for (const id of ["RF-0019", "RF-0020"]) {
      const article = tile(shown, id);
      expect([id, article.querySelector("img, audio, video")]).toEqual([id, null]);
      expect([id, within(article).getAllByRole("link").map((link) => link.getAttribute("href"))]).toEqual([id, [`/api/projects/alpha/blobs/${article === tile(shown, "RF-0019") ? aB3("4") : aB3("5")}`]]);
    }
  });

  it("plays a sound with <audio controls preload=\"none\"> and a video with <video controls preload=\"metadata\">, each with its download", async () => {
    const shown = await part(client());
    const audio = tile(shown, "RF-0012").querySelector("audio");
    expect([audio?.hasAttribute("controls"), audio?.getAttribute("preload"), audio?.getAttribute("src")]).toEqual([true, "none", `/api/projects/alpha/blobs/${aB3("d")}`]);
    expect(tile(shown, "RF-0012").textContent).toContain("2.0 s");
    const video = tile(shown, "RF-0013").querySelector("video");
    expect([video?.hasAttribute("controls"), video?.getAttribute("preload"), video?.getAttribute("src")]).toEqual([true, "metadata", `/api/projects/alpha/blobs/${aB3("e")}`]);
    expect(within(tile(shown, "RF-0013")).getByRole("link", { name: "Download" }).getAttribute("href")).toBe(`/api/projects/alpha/blobs/${aB3("e")}`);
  });

  it("offers a model, a .blend with the daemon's warning, and a PDF for download, nothing drawn", async () => {
    const shown = await part(client());
    for (const [id, name, seed] of [
      ["RF-0014", "crane.glb", "1"],
      ["RF-0015", "quay.blend", "2"],
      ["RF-0016", "tides.pdf", "3"],
    ] as const) {
      const article = tile(shown, id);
      expect([id, article.querySelector("img, audio, video")]).toEqual([id, null]);
      const link = within(article).getByRole("link", { name: `Download ${name}` });
      expect([id, link.getAttribute("href"), link.hasAttribute("download")]).toEqual([id, `/api/projects/alpha/blobs/${aB3(seed)}`, true]);
    }
    expect(tile(shown, "RF-0014").textContent).toContain("16 triangles");
    expect(tile(shown, "RF-0015").textContent).toContain("Blender can run the scripts it holds: open it with Auto Run Python Scripts off");
    expect(tile(shown, "RF-0016").textContent).not.toContain("Blender");
  });

  it("opens a link in a new tab with no opener, no referrer and no follow, its address whole; an address not http(s) stays text", async () => {
    const shown = await part(client([...tiles(), aLinkReference({ id: "RF-0021", title: "Odd", url: "javascript:void(0)" })]));
    const anchor = within(tile(shown, "RF-0010")).getByRole("link", { name: "https://example.org/board" });
    expect([anchor.getAttribute("href"), anchor.getAttribute("target"), anchor.getAttribute("rel")]).toEqual(["https://example.org/board", "_blank", "noopener noreferrer nofollow"]);
    expect(within(tile(shown, "RF-0021")).queryAllByRole("link")).toEqual([]);
    expect(tile(shown, "RF-0021").textContent).toContain("javascript:void(0)");
  });

  it("shows people's text as text, a lost file and a tombstone in words, the tombstone with no act", async () => {
    const shown = await part(client());
    const image = tile(shown, "RF-0011");
    expect(image.textContent).toContain("The light on the water, <b>not</b> the boats");
    expect(image.querySelector(".reference-matters b")).toBeNull();
    expect(tile(shown, "RF-0017").textContent).toContain("Lost with the host it was uploaded to.");
    expect(tile(shown, "RF-0017").querySelector("img, a")).toBeNull();
    const gone = tile(shown, "RF-0018");
    expect(gone.textContent).toContain("Deleted by lev, 2026-11-23 11:00 UTC");
    expect(within(gone).queryAllByRole("button")).toEqual([]);
  });

  it("draws a thumbnail from the file only when it reaches agents as it is; a large original with no copy is an Open image button that loads nothing until pressed (M: the original loaded as a thumbnail)", async () => {
    const big = aReference({ id: "RF-0022", title: "Quay at noon", b3: aB3("6"), size: 2_202_009, meta: { width: 4032, height: 3024, duration_ms: null, triangles: null } });
    const small = aReference({ id: "RF-0023", title: "Bollard", b3: aB3("7") });
    const shown = await part(client([big, small]));
    const large = tile(shown, "RF-0022");
    expect(large.querySelector("img")).toBeNull();
    expect(Array.from(document.querySelectorAll("img"), (img) => img.getAttribute("src"))).not.toContain(`/api/projects/alpha/blobs/${aB3("6")}`);
    const open = within(large).getByRole("button", { name: "Open image (4032×3024, 2.1 MiB)" });
    // 1200x800, 300 KiB: agent-ready as it is, so its own thumbnail, sized from its facts, decoded off the main thread.
    const img = tile(shown, "RF-0023").querySelector("img");
    expect([img?.getAttribute("src"), img?.getAttribute("width"), img?.getAttribute("height"), img?.getAttribute("decoding"), img?.getAttribute("loading")]).toEqual([
      `/api/projects/alpha/blobs/${aB3("7")}`,
      "1200",
      "800",
      "async",
      "lazy",
    ]);
    fireEvent.click(open);
    const viewer = await screen.findByRole("dialog", { name: "Quay at noon" });
    expect(viewer.querySelector("img")?.getAttribute("src")).toBe(`/api/projects/alpha/blobs/${aB3("6")}`);
    fireEvent.click(within(viewer).getByRole("button", { name: "Close" }));
    await waitFor(() => {
      expect(document.activeElement).toBe(open);
    });
  });

  it("opens an image past 64 megapixels to its facts and Download alone, never drawn; reserves no size for a copy or a JPEG (M: the viewer draws any original)", async () => {
    const vast = aReference({ id: "RF-0024", title: "Survey", b3: aB3("8"), agent_b3: aB3("9"), size: 300 * 1_048_576, meta: { width: 10000, height: 10000, duration_ms: null, triangles: null } });
    const photo = aReference({ id: "RF-0025", title: "Phone photo", b3: aB3("5"), media: "image/jpeg", name: "photo.jpg" });
    const shown = await part(client([vast, photo]));
    const thumb = tile(shown, "RF-0024").querySelector("img");
    expect([thumb?.getAttribute("src"), thumb?.hasAttribute("width"), thumb?.hasAttribute("height")]).toEqual([`/api/projects/alpha/blobs/${aB3("9")}`, false, false]);
    const jpeg = tile(shown, "RF-0025").querySelector("img");
    expect([jpeg?.getAttribute("src"), jpeg?.hasAttribute("width")]).toEqual([`/api/projects/alpha/blobs/${aB3("5")}`, false]);
    fireEvent.click(within(tile(shown, "RF-0024")).getByRole("button", { name: "Open Survey" }));
    const viewer = await screen.findByRole("dialog", { name: "Survey" });
    expect(viewer.querySelector("img")).toBeNull();
    expect(viewer.textContent).toContain("10000×10000, 300.0 MiB: too large to draw here; download it to view it.");
    expect(within(viewer).getByRole("link", { name: "Download" }).getAttribute("href")).toBe(`/api/projects/alpha/blobs/${aB3("8")}`);
  });

  it("pages back by next_before: Load older reads the page before, drawn below", async () => {
    const stub = client([]);
    stub.getReferences.mockImplementation((_project, options) =>
      Promise.resolve(
        options.before === undefined
          ? { on: { kind: "card", id: "C-0007" }, total: 2, references: [aLinkReference({ id: "RF-0030" })], next_before: "RF-0030", notes: [] }
          : { on: { kind: "card", id: "C-0007" }, total: 2, references: [aLinkReference({ id: "RF-0004", title: "Older" })], next_before: null, notes: [] },
      ),
    );
    const shown = await part(stub);
    fireEvent.click(within(shown).getByRole("button", { name: "Load older" }));
    await within(shown).findByText("Older");
    expect(argsOf(stub.getReferences)).toEqual([
      ["alpha", { on: "C-0007" }],
      ["alpha", { on: "C-0007", before: "RF-0030" }],
    ]);
    expect(Array.from(shown.querySelectorAll("[data-reference]"), (article) => article.getAttribute("data-reference"))).toEqual(["RF-0030", "RF-0004"]);
  });
});

describe("Delete, through a dialog (RF21: ActOff without delete_reference)", () => {
  it("on a hub, stands on one's own and stays off on another's with its line when `allowed` lacks delete_reference (M: delete without ActOff)", async () => {
    const stub = onHub(client(), "lev", ["stage", "accept", "clear", "delete_comment"]);
    const shown = await part(stub);
    await waitFor(() => {
      expect(deleteOf(tile(shown, "RF-0011")).getAttribute("aria-disabled")).toBe("true");
    });
    const another = tile(shown, "RF-0011");
    expect(Array.from(another.children).some((child) => child.textContent === "Your login deletes only its own references on this project.")).toBe(true);
    fireEvent.click(deleteOf(another));
    expect(screen.queryByRole("dialog", { name: "Delete RF-0011?" })).toBeNull();
    // lev's own link: on, no line.
    expect(deleteOf(tile(shown, "RF-0010")).getAttribute("aria-disabled")).toBe("false");
    expect(tile(shown, "RF-0010").textContent).not.toContain("deletes only its own references");
    expect(stub.deleteReference).not.toHaveBeenCalled();
  });

  it("says a link's delete without a file: its title, what matters and address are erased", async () => {
    const shown = await part(client());
    fireEvent.click(deleteOf(tile(shown, "RF-0010")));
    const dialog = await screen.findByRole("dialog", { name: "Delete RF-0010?" });
    expect(dialog.textContent).toContain("The title, what matters and the address of lev's link are erased");
    expect(dialog.textContent).not.toContain("Its file goes");
  });

  it("on a hub whose `allowed` lists delete_reference (an owner), stands on another's", async () => {
    const shown = await part(onHub(client(), "lev", ["stage", "accept", "clear", "group_clear", "confirm", "delete_comment", "delete_reference"]));
    await waitFor(() => {
      expect(deleteOf(tile(shown, "RF-0011")).getAttribute("aria-disabled")).toBe("false");
    });
    expect(shown.textContent).not.toContain("deletes only its own references");
  });

  it("locally deletes any after the dialog, which says the file goes when no other reference names it; a refusal keeps it with the daemon's words", async () => {
    const stub = client();
    const shown = await part(stub);
    const trigger = deleteOf(tile(shown, "RF-0011"));
    fireEvent.click(trigger);
    let dialog = await screen.findByRole("dialog", { name: "Delete RF-0011?" });
    expect(dialog.textContent).toContain("Its file goes when no other reference names it.");
    expect(document.activeElement?.textContent).toBe("Cancel");
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(stub.deleteReference).not.toHaveBeenCalled();
    await waitFor(() => {
      expect(document.activeElement).toBe(trigger);
    });
    stub.deleteReference.mockRejectedValueOnce(new ClientError({ status: 403, message: "refused: RF-0011 is mira's: only its author or an `owner` deletes it" }));
    fireEvent.click(trigger);
    dialog = await screen.findByRole("dialog", { name: "Delete RF-0011?" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Delete" }));
    expect((await within(dialog).findByRole("alert")).textContent).toContain("refused: RF-0011 is mira's: only its author or an `owner` deletes it");
    fireEvent.click(within(dialog).getByRole("button", { name: "Delete" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Delete RF-0011?" })).toBeNull();
    });
    expect(argsOf(stub.deleteReference)).toEqual([
      ["alpha", "RF-0011"],
      ["alpha", "RF-0011"],
    ]);
    await waitFor(() => {
      expect(tile(shown, "RF-0011").textContent).toContain("Deleted by local");
    });
  });
});

/** Stubs the browser's drawing: each decode the size asked, each encoding `size` bytes of the type asked; the decode's stub returned. */
function stubImage(size: number) {
  const decode = vi.fn((_source: ImageBitmapSource, options?: ImageBitmapOptions) => Promise.resolve({ width: options?.resizeWidth ?? 0, height: options?.resizeHeight ?? 0, close: () => undefined }));
  vi.stubGlobal("createImageBitmap", decode);
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(() => ({ drawImage: () => undefined }) as unknown as CanvasRenderingContext2D);
  vi.spyOn(HTMLCanvasElement.prototype, "toBlob").mockImplementation((callback, type) => {
    callback(new Blob([new Uint8Array(size)], { type: type ?? "" }));
  });
  return decode;
}

/** Opens Add a file… on the part and picks `picked`. */
async function pick(shown: HTMLElement, picked: File): Promise<HTMLElement> {
  fireEvent.click(within(shown).getByRole("button", { name: "Add a file…" }));
  const dialog = await screen.findByRole("dialog", { name: "Add a file" });
  fireEvent.change(within(dialog).getByLabelText("Choose a file"), { target: { files: [picked] } });
  return dialog;
}

/** A stored blob for a picked file, by its type: a copy the daemon finds agent-ready, the 4000 px original not. */
function storedFor(blob: Blob): BlobStored {
  return blob.type === "image/webp"
    ? aBlobStored({ b3: aB3("c"), media: "image/webp", size: blob.size, meta: { width: 1568, height: 1176, duration_ms: null, triangles: null } })
    : aBlobStored({ b3: aB3("f"), size: blob.size, agent_ready: false, meta: { width: 4000, height: 3000, duration_ms: null, triangles: null } });
}

describe("Add a file… (RF21)", () => {
  it("puts a 4000 px image's copy before the original and names it in the attach (M: no downscale; M: the original posted as its own copy)", async () => {
    window.localStorage.setItem("specengine.board.as", "mira");
    stubImage(400 * 1024);
    const stub = client([]);
    stub.putBlob.mockImplementation((_project, blob, onProgress) => {
      onProgress(blob.size, blob.size);
      return Promise.resolve(storedFor(blob));
    });
    const shown = await part(stub);
    const original = fileOf(pngBytes(4000, 3000), "harbour.png", "image/png");
    const dialog = await pick(shown, original);
    await within(dialog).findByText("A copy for agents goes with it, 1568×1176, 400.0 KiB.");
    expect(stub.putBlob.mock.calls.map(([, blob]) => [blob === original, blob.type])).toEqual([
      [false, "image/webp"],
      [true, "image/png"],
    ]);
    fireEvent.change(within(dialog).getByLabelText("Title"), { target: { value: "Harbour at dusk" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Attach" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Add a file" })).toBeNull();
    });
    expect(argsOf(stub.addReference)).toEqual([["alpha", { on: "C-0007", b3: aB3("f"), agent_b3: aB3("c"), name: "harbour.png", title: "Harbour at dusk", matters: null, as: "mira" }]]);
    const body = argsOf(stub.addReference)[0]?.[1] as { b3: string; agent_b3: string };
    expect(body.agent_b3).not.toBe(body.b3);
    await within(shown).findByText("Harbour at dusk");
  });

  it("never decodes a 60000x60000 PNG its header declares: no copy, said so, the file still uploaded alone (M: decoded before its size is read)", async () => {
    const decode = stubImage(10);
    const stub = client([]);
    const shown = await part(stub);
    const huge = fileOf(pngBytes(60000, 60000), "huge.png", "image/png");
    const dialog = await pick(shown, huge);
    await within(dialog).findByText(/^Uploaded huge\.png/);
    expect(decode).not.toHaveBeenCalled();
    expect(dialog.querySelector(".reference-copy-note")?.textContent).toContain("No copy for agents: the image is 60000×60000, past 64 megapixels");
    expect(stub.putBlob.mock.calls.map(([, blob]) => blob)).toEqual([huge]);
  });

  it("attaches once more without the copy when the daemon refuses it, so the file lands (M: the copy's refusal shown and nothing attached)", async () => {
    stubImage(400 * 1024);
    const stub = client([]);
    stub.putBlob.mockImplementation((_project, blob, onProgress) => {
      onProgress(blob.size, blob.size);
      return Promise.resolve(storedFor(blob));
    });
    stub.addReference.mockRejectedValueOnce(new ClientError({ status: 400, message: "spec: agent_b3: the copy is 1568x1175, the image 4000x3000: a copy keeps the image's aspect" }));
    const shown = await part(stub);
    const dialog = await pick(shown, fileOf(pngBytes(4000, 3000), "harbour.png", "image/png"));
    await within(dialog).findByText(/^Uploaded harbour\.png/);
    fireEvent.click(within(dialog).getByRole("button", { name: "Attach" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Add a file" })).toBeNull();
    });
    expect(argsOf(stub.addReference).map(([, body]) => (body as { agent_b3: string | null }).agent_b3)).toEqual([aB3("c"), null]);
    expect(stub.putBlob).toHaveBeenCalledTimes(2);
  });

  it("says each step as it starts and each quarter it passes, and its end once, with no live region of its own", async () => {
    const stub = client([]);
    let finish: (stored: BlobStored) => void = () => undefined;
    let report: UploadProgress = () => undefined;
    stub.putBlob.mockImplementation(
      (_project: string, _blob: Blob, onProgress: UploadProgress) =>
        new Promise<BlobStored>((resolve) => {
          report = onProgress;
          finish = resolve;
        }),
    );
    const shown = await part(stub);
    const dialog = await pick(shown, new File([new Uint8Array(200)], "tides.pdf", { type: "application/pdf" }));
    const said = () => document.querySelector('[data-live-region] [aria-live="polite"]')?.textContent;
    await waitFor(() => {
      expect(said()).toBe("Uploading tides.pdf");
    });
    act(() => {
      report(60, 200);
    });
    expect(said()).toBe("Uploading tides.pdf: 25%");
    // Each announcement is a node of its own: the same quarter is not said again (M: said at every progress event).
    const spokenNode = () => document.querySelector('[data-live-region] [aria-live="polite"] p');
    const quarter = spokenNode();
    act(() => {
      report(70, 200);
    });
    expect(spokenNode()).toBe(quarter);
    act(() => {
      finish(aBlobStored({ b3: aB3("p"), media: "application/pdf", class: "pdf", inline: false, agent_ready: false, needs_matters: true }));
    });
    await within(dialog).findByText(/^Uploaded tides\.pdf/);
    expect(said()).toBe("Uploaded tides.pdf");
    expect(dialog.querySelector(".reference-uploaded")?.getAttribute("role")).toBeNull();
  });

  it("takes a file dropped on its zone alone: a drop anywhere else on the dialog is held, never opened by the page (M: the drop outside the zone left to the browser)", async () => {
    const stub = client([]);
    const shown = await part(stub);
    fireEvent.click(within(shown).getByRole("button", { name: "Add a file…" }));
    const dialog = await screen.findByRole("dialog", { name: "Add a file" });
    const file = new File([new Uint8Array(8)], "tides.pdf", { type: "application/pdf" });
    const transfer = () => ({ dataTransfer: { files: [file], dropEffect: "" } });
    const title = within(dialog).getByLabelText("Title");
    expect(fireEvent.dragOver(title, transfer())).toBe(false);
    expect(fireEvent.drop(title, transfer())).toBe(false);
    expect(stub.putBlob).not.toHaveBeenCalled();
    const zone = dialog.querySelector<HTMLElement>(".reference-drop");
    if (zone === null) {
      throw new Error("no drop zone");
    }
    fireEvent.drop(zone, transfer());
    await within(dialog).findByText(/^Uploaded tides\.pdf/);
    expect(stub.putBlob).toHaveBeenCalledTimes(1);
  });

  it("uploads an image within the agents' limits alone, naming no copy", async () => {
    const decode = stubImage(10);
    const stub = client([]);
    const shown = await part(stub);
    const dialog = await pick(shown, fileOf(pngBytes(1200, 800), "small.png", "image/png"));
    await within(dialog).findByText(/^Uploaded small\.png/);
    expect(stub.putBlob).toHaveBeenCalledTimes(1);
    // Within both limits by its header: never decoded, nothing said of a copy.
    expect(decode).not.toHaveBeenCalled();
    expect(dialog.querySelector(".reference-copy-note")).toBeNull();
    fireEvent.click(within(dialog).getByRole("button", { name: "Attach" }));
    await waitFor(() => {
      expect(stub.addReference).toHaveBeenCalled();
    });
    expect(argsOf(stub.addReference)[0]?.[1]).toMatchObject({ agent_b3: null });
  });

  it("shows the upload's progress as onProgress reports it, and Cancel upload aborts it, attaching nothing", async () => {
    const stub = client([]);
    const signals: AbortSignal[] = [];
    stub.putBlob.mockImplementation(
      (_project: string, _blob: Blob, onProgress: UploadProgress, signal: AbortSignal) =>
        new Promise<BlobStored>((_resolve, reject) => {
          signals.push(signal);
          onProgress(50, 200);
          signal.addEventListener("abort", () => {
            reject(new DOMException("The upload was cancelled", "AbortError"));
          });
        }),
    );
    const shown = await part(stub);
    const dialog = await pick(shown, new File([new Uint8Array(200)], "tides.pdf", { type: "application/pdf" }));
    const bar = await within(dialog).findByRole("progressbar", { name: "Uploading tides.pdf" });
    expect([bar.getAttribute("value"), bar.getAttribute("max"), bar.getAttribute("aria-valuetext")]).toEqual(["50", "200", "25%"]);
    fireEvent.click(within(dialog).getByRole("button", { name: "Attach" }));
    expect(within(dialog).getByRole("alert").textContent).toBe("Choose a file and wait for its upload to end first.");
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel upload" }));
    expect(signals.map((signal) => signal.aborted)).toEqual([true]);
    await waitFor(() => {
      expect(within(dialog).queryByRole("progressbar")).toBeNull();
    });
    expect(stub.addReference).not.toHaveBeenCalled();
    // Leaving the dialog mid-upload aborts it too.
    fireEvent.change(within(dialog).getByLabelText("Choose a file"), { target: { files: [new File([new Uint8Array(8)], "again.pdf", { type: "application/pdf" })] } });
    await within(dialog).findByRole("progressbar");
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => {
      expect(signals.map((signal) => signal.aborted)).toEqual([true, true]);
    });
  });

  it("keeps the dialog and the uploaded file on a 400: the next Attach sends nothing again; 'what matters' required by the daemon's answer", async () => {
    const stub = client([]);
    stub.putBlob.mockImplementation((_project, blob) => Promise.resolve(aBlobStored({ b3: aB3("p"), size: blob.size, media: "application/pdf", class: "pdf", inline: false, agent_ready: false, needs_matters: true, meta: { width: null, height: null, duration_ms: null, triangles: null } })));
    stub.addReference.mockRejectedValueOnce(new ClientError({ status: 400, message: "spec: title: 300 B is over 256 B" }));
    const shown = await part(stub);
    const dialog = await pick(shown, new File([new Uint8Array(32)], "tides.pdf", { type: "application/pdf" }));
    await within(dialog).findByText(/^Uploaded tides\.pdf/);
    // Blank "what matters" for a PDF: stopped here, nothing posted.
    fireEvent.click(within(dialog).getByRole("button", { name: "Attach" }));
    expect(within(dialog).getByRole("alert").textContent).toBe("Say what matters here: audio, video, models and PDF need it.");
    expect(stub.addReference).not.toHaveBeenCalled();
    fireEvent.change(within(dialog).getByLabelText("What matters here (required)"), { target: { value: "The spring tides." } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Attach" }));
    expect((await within(dialog).findByRole("alert")).textContent).toContain("spec: title: 300 B is over 256 B");
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("What matters here (required)").value).toBe("The spring tides.");
    fireEvent.click(within(dialog).getByRole("button", { name: "Attach" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Add a file" })).toBeNull();
    });
    expect(stub.putBlob).toHaveBeenCalledTimes(1);
    expect(argsOf(stub.addReference).map(([, body]) => body)).toEqual([
      { on: "C-0007", b3: aB3("p"), agent_b3: null, name: "tides.pdf", title: null, matters: "The spring tides.", as: null },
      { on: "C-0007", b3: aB3("p"), agent_b3: null, name: "tides.pdf", title: null, matters: "The spring tides.", as: null },
    ]);
  });

  it("uploads a file once more when the attach's 404 says it is gone, then attaches again", async () => {
    const stub = client([]);
    stub.putBlob.mockImplementation((_project, blob) => Promise.resolve(aBlobStored({ b3: aB3("g"), size: blob.size, media: "model/gltf-binary", class: "model", inline: false, agent_ready: false, needs_matters: true })));
    stub.addReference.mockRejectedValueOnce(new ClientError({ status: 404, message: `blob ${aB3("g")} is not stored here: upload it again` }));
    const shown = await part(stub);
    const dialog = await pick(shown, new File([new Uint8Array(32)], "crane.glb"));
    await within(dialog).findByText(/^Uploaded crane\.glb/);
    fireEvent.change(within(dialog).getByLabelText("What matters here (required)"), { target: { value: "Its reach." } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Attach" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Add a file" })).toBeNull();
    });
    expect([stub.putBlob.mock.calls.length, stub.addReference.mock.calls.length]).toEqual([2, 2]);
  });

  it("refuses a pick past 256 MiB before a byte is sent, and marks 'what matters' required for a sound by its browser type", async () => {
    const stub = client([]);
    stub.putBlob.mockImplementation(() => new Promise<BlobStored>(() => undefined));
    const shown = await part(stub);
    const huge = new File([new Uint8Array(8)], "harbour.wav", { type: "audio/wav" });
    Object.defineProperty(huge, "size", { value: 300 * MIB });
    const dialog = await pick(shown, huge);
    expect((await within(dialog).findByRole("alert")).textContent).toBe("harbour.wav is 300.0 MiB; a file here is at most 256 MiB. Nothing was sent.");
    expect(stub.putBlob).not.toHaveBeenCalled();
    fireEvent.change(within(dialog).getByLabelText("Choose a file"), { target: { files: [new File([new Uint8Array(8)], "foghorn.wav", { type: "audio/wav" })] } });
    await within(dialog).findByRole("progressbar");
    expect(within(dialog).getByLabelText("What matters here (required)").getAttribute("aria-required")).toBe("true");
  });

  it("says an upload's refusal in the daemon's words and offers it again", async () => {
    const stub = client([]);
    stub.putBlob.mockRejectedValueOnce(new ClientError({ status: 415, message: "refused: the upload: this file is HTML, which is never stored here" }));
    const shown = await part(stub);
    const dialog = await pick(shown, new File([new Uint8Array(8)], "harbour.png", { type: "text/html" }));
    const refusal = await within(dialog).findByRole("alert");
    expect(refusal.textContent).toContain("The upload was refused");
    expect(refusal.textContent).toContain("refused: the upload: this file is HTML, which is never stored here");
    fireEvent.click(within(refusal).getByRole("button", { name: "Upload again" }));
    await within(dialog).findByText(/^Uploaded harbour\.png/);
    expect(stub.putBlob).toHaveBeenCalledTimes(2);
  });
});

describe("Add a link…", () => {
  it("posts the address, the title and 'what matters' under the acting name; a blank address is stopped here", async () => {
    window.localStorage.setItem("specengine.board.as", "lev");
    const stub = client([]);
    const shown = await part(stub);
    fireEvent.click(within(shown).getByRole("button", { name: "Add a link…" }));
    const dialog = await screen.findByRole("dialog", { name: "Add a link" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Add the link" }));
    expect(within(dialog).getByRole("alert").textContent).toBe("Type the link's address first.");
    fireEvent.change(within(dialog).getByLabelText("Address"), { target: { value: " https://example.org/board " } });
    fireEvent.change(within(dialog).getByLabelText("Title"), { target: { value: "Moodboard" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Add the link" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Add a link" })).toBeNull();
    });
    expect(argsOf(stub.addReference)).toEqual([["alpha", { on: "C-0007", url: "https://example.org/board", title: "Moodboard", matters: null, as: "lev" }]]);
    expect(within(shown).getByRole("link", { name: "https://example.org/board" })).toBeTruthy();
  });
});

describe("live and in Russian", () => {
  it("reads the target again on its reference.* events, never another target's", async () => {
    const stub = client();
    let emit: (event: QueueEvent) => void = () => undefined;
    stub.subscribe = vi.fn((_project: string, onEvent: (event: QueueEvent) => void) => {
      emit = onEvent;
      return () => undefined;
    });
    const shown = await part(stub);
    const reads = () => stub.getReferences.mock.calls.length;
    const before = reads();
    stub.state.references.push(aLinkReference({ id: "RF-0040", title: "A newer link" }));
    act(() => {
      emit({ seq: 40, type: "reference.created", payload: { id: "RF-0040", on: "C-0007", as: "mira" } });
    });
    await within(shown).findByText("A newer link");
    expect(reads()).toBe(before + 1);
    act(() => {
      emit({ seq: 41, type: "reference.created", payload: { id: "RF-0041", on: "C-0999", as: "mira" } });
    });
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    expect(reads()).toBe(before + 1);
    act(() => {
      emit({ seq: 42, type: "reference.deleted", payload: { id: "RF-0010", as: "lev" } });
    });
    await waitFor(() => {
      expect(reads()).toBe(before + 2);
    });
  });

  it("reads an open agent task again on a reference.* event: its package carries its cards' references", async () => {
    const stub = client();
    stub.getTasks.mockResolvedValue({ tasks: [aTaskEntry({ id: "T-0001" })], notes: [] });
    stub.getTask.mockImplementation((_project, id) => Promise.resolve(aTaskPackage({ id })));
    let emit: (event: QueueEvent) => void = () => undefined;
    stub.subscribe = vi.fn((_project: string, onEvent: (event: QueueEvent) => void) => {
      emit = onEvent;
      return () => undefined;
    });
    renderApp(stub, "#/alpha/tasks/T-0001");
    // Agent tasks is a chunk of its own: its first read waits for it.
    await waitFor(
      () => {
        expect(stub.getTask).toHaveBeenCalled();
      },
      { timeout: 5000 },
    );
    const reads = stub.getTask.mock.calls.length;
    act(() => {
      emit({ seq: 80, type: "reference.deleted", payload: { id: "RF-0031", as: "mira" } });
    });
    await waitFor(() => {
      expect(stub.getTask.mock.calls.length).toBe(reads + 1);
    });
  });

  it("words its chrome from ru.ts, the people's words as written", async () => {
    const shown = await part(client());
    act(() => {
      switchLanguage("ru");
    });
    expect(within(shown).getByRole("heading", { level: 3 }).textContent).toBe(ru.references.titleCount_many.replace("{{count}}", "10"));
    expect(within(shown).getByRole("button", { name: ru.references.addFile })).toBeTruthy();
    expect(shown.textContent).toContain("The light on the water");
  });
});

describe("the document view's and the proposal pane's part", () => {
  it("reads a node's references under its canonical ID, a part at the page's level", async () => {
    const stub = stubClient();
    stub.getNode.mockImplementation((_project, ref) => Promise.resolve({ ref, reason: null, notes: [], nodes: [aNode({ id: "MEC-RAIL" })] }));
    stub.state.references.push(aLinkReference({ id: "RF-0050", on: { kind: "node", id: "MEC-RAIL" }, on_rev: 3, title: "Rail photos" }));
    renderApp(stub, "#/alpha/tree/RAIL-OLD/page");
    await screen.findByText("Rail photos");
    const shown = screen.getByRole("region", { name: "References" });
    expect(within(shown).getByRole("heading", { level: 2 }).textContent).toBe("References · 1");
    expect(argsOf(stub.getReferences)).toEqual([["alpha", { on: "MEC-RAIL" }]]);
    expect(tile(shown, "RF-0050").textContent).toContain("on rev 3");
  });

  it("stands under an open proposal and under a closed one", async () => {
    const closed = aProposal({ id: "PR-0032", kind: "question", status: "applied", target_id: "MEC-C", target_ids: ["MEC-C"], decided_by: "mira", decided_at: "2026-11-21T10:00:00Z" });
    const stub = stubClient([aProposal({ id: "PR-0031", kind: "question", target_id: "MEC-A", target_ids: ["MEC-A"], summary: "Rail?" }), closed]);
    stub.getInbox.mockImplementation(() => Promise.resolve({ proposals: [entryOf(aProposal({ id: "PR-0031", kind: "question", target_id: "MEC-A", target_ids: ["MEC-A"], summary: "Rail?" }))], notes: [] }));
    stub.state.references.push(aLinkReference({ id: "RF-0060", on: { kind: "proposal", id: "PR-0031" }, title: "The rail log" }), aLinkReference({ id: "RF-0061", on: { kind: "proposal", id: "PR-0032" }, title: "Read before deciding" }));
    renderApp(stub, "#/alpha/inbox/PR-0031");
    await screen.findByText("The rail log");
    expect(within(screen.getByRole("region", { name: "References" })).getByRole("heading", { level: 3 }).textContent).toBe("References · 1");
    window.location.hash = "#/alpha/inbox/PR-0032";
    await screen.findByText("Read before deciding");
    expect(screen.getByText("Read before deciding").closest("[data-closed]")?.getAttribute("data-closed")).toBe("PR-0032");
  });
});
