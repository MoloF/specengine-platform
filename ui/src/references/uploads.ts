import type { SpecEngineClient } from "../api/client";
import type { BlobStored, ReferenceFileBody } from "../api/types";
import { takesCopy } from "./labels";

// A file's way to the store (docs/features/work-references.md "UI", Add a file…): its agent copy first,
// when one was made, then the file, each one `putBlob` of the one client interface; at most two
// uploads at once from a page, the rest waiting their turn; Cancel aborts the one running or the one
// waiting. The attach that follows names what came back.

/** The most uploads a page runs at once (`docs/features/work-references.md` "UI"); a hub's login has two slots. */
export const PAGE_UPLOADS = 2;

let running = 0;
const waiting: (() => void)[] = [];

/** What Cancel ends a waiting upload with, as an aborted request does: its name says it, no message. */
function cancelled(): DOMException {
  return new DOMException("", "AbortError");
}

/** Takes one of the page's upload slots, waiting for one when all are taken; rejects when `signal` aborts first. */
function takeSlot(signal: AbortSignal): Promise<void> {
  if (signal.aborted) {
    return Promise.reject(cancelled());
  }
  if (running < PAGE_UPLOADS) {
    running += 1;
    return Promise.resolve();
  }
  return new Promise((resolve, reject) => {
    const go = () => {
      signal.removeEventListener("abort", stop);
      running += 1;
      resolve();
    };
    const stop = () => {
      const at = waiting.indexOf(go);
      if (at >= 0) {
        waiting.splice(at, 1);
      }
      reject(cancelled());
    };
    waiting.push(go);
    signal.addEventListener("abort", stop, { once: true });
  });
}

function giveSlot(): void {
  running -= 1;
  waiting.shift()?.();
}

/** One call within the page's slots: it waits for one, runs, and gives it back however it ends. */
export async function withSlot<T>(signal: AbortSignal, run: () => Promise<T>): Promise<T> {
  await takeSlot(signal);
  try {
    return await run();
  } finally {
    giveSlot();
  }
}

/** Which of a file's uploads is running: its agent copy's or its own. */
export type UploadStep = "copy" | "file";

/** How far an upload has gone, step by step. */
export type StepProgress = (step: UploadStep, sent: number, total: number) => void;

/** What the store answered for a file and, when one was sent, its copy. */
export interface Uploaded {
  file: BlobStored;
  copy: BlobStored | null;
}

/** Uploads a file, its agent copy first (`docs/features/work-references.md` "The agent copy"), each within the page's slots. */
export async function uploadAll(client: SpecEngineClient, project: string, file: Blob, copy: Blob | null, onProgress: StepProgress, signal: AbortSignal): Promise<Uploaded> {
  const copied =
    copy === null
      ? null
      : await withSlot(signal, () =>
          client.putBlob(
            project,
            copy,
            (sent, total) => {
              onProgress("copy", sent, total);
            },
            signal,
          ),
        );
  const stored = await withSlot(signal, () =>
    client.putBlob(
      project,
      file,
      (sent, total) => {
        onProgress("file", sent, total);
      },
      signal,
    ),
  );
  return { file: stored, copy: copied };
}

/** The attach's body: the file's hash, its copy's only where the daemon does not find the file agent-ready, the texts as given (blank ones null). */
export function attachBody(on: string, uploaded: Uploaded, fields: { name: string; title: string; matters: string; as: string | null }): ReferenceFileBody {
  const title = fields.title.trim();
  const matters = fields.matters.trim();
  return {
    on,
    b3: uploaded.file.b3,
    agent_b3: uploaded.copy !== null && takesCopy(uploaded.file) ? uploaded.copy.b3 : null,
    name: fields.name,
    title: title === "" ? null : title,
    matters: matters === "" ? null : matters,
    as: fields.as,
  };
}
