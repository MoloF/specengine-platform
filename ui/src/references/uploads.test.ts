import { describe, expect, it } from "vitest";
import { aB3, aBlobStored } from "../test/builders";
import { stubClient } from "../test/stubClient";
import { attachBody, PAGE_UPLOADS, uploadAll, withSlot } from "./uploads";

// docs/features/work-references.md "UI", Add a file…: at most two uploads at once from a page, the
// rest waiting their turn, Cancel taking one out of the line; a file's agent copy uploaded before it;
// the attach naming the copy only where the daemon does not find the file agent-ready.

/** A call held open until the test lets it end. */
function held() {
  let end: () => void = () => undefined;
  const done = new Promise<void>((resolve) => {
    end = resolve;
  });
  return {
    done,
    end: () => {
      end();
    },
  };
}

async function settled(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

describe("the page's upload slots", () => {
  it("runs two at once, the third when one ends (M: no limit)", async () => {
    expect(PAGE_UPLOADS).toBe(2);
    const calls = [held(), held(), held()];
    const started: number[] = [];
    const runs = calls.map((call, index) =>
      withSlot(new AbortController().signal, async () => {
        started.push(index);
        await call.done;
        return index;
      }),
    );
    await settled();
    expect(started).toEqual([0, 1]);
    calls[1]?.end();
    await settled();
    expect(started).toEqual([0, 1, 2]);
    calls[0]?.end();
    calls[2]?.end();
    expect(await Promise.all(runs)).toEqual([0, 1, 2]);
  });

  it("drops a waiting upload Cancel aborts, never running it, and gives a slot back when a call fails", async () => {
    const first = held();
    const second = held();
    const running = [withSlot(new AbortController().signal, () => first.done), withSlot(new AbortController().signal, () => second.done)];
    const cancel = new AbortController();
    let ran = false;
    const waiting = withSlot(cancel.signal, () => {
      ran = true;
      return Promise.resolve();
    });
    cancel.abort();
    await expect(waiting).rejects.toMatchObject({ name: "AbortError" });
    first.end();
    second.end();
    await Promise.all(running);
    expect(ran).toBe(false);
    await expect(withSlot(new AbortController().signal, () => Promise.reject(new Error("refused")))).rejects.toThrow("refused");
    // Both slots free again: two run at once.
    const both = [held(), held()];
    let count = 0;
    const again = both.map((call) =>
      withSlot(new AbortController().signal, async () => {
        count += 1;
        await call.done;
      }),
    );
    await settled();
    expect(count).toBe(2);
    both.forEach((call) => {
      call.end();
    });
    await Promise.all(again);
  });
});

describe("a file's uploads and its attach", () => {
  it("uploads the agent copy first, then the file (M: the original first)", async () => {
    const stub = stubClient();
    const file = new File([new Uint8Array(32)], "harbour.png", { type: "image/png" });
    const copy = new Blob([new Uint8Array(8)], { type: "image/webp" });
    const steps: string[] = [];
    const uploaded = await uploadAll(stub, "alpha", file, copy, (step) => steps.push(step), new AbortController().signal);
    expect(stub.putBlob.mock.calls.map(([, blob]) => blob)).toEqual([copy, file]);
    expect([...new Set(steps)]).toEqual(["copy", "file"]);
    expect([uploaded.copy?.b3, uploaded.file.b3]).toEqual([aB3("1"), aB3("2")]);
  });

  it("names the copy only for an image the daemon does not find agent-ready; blank texts go as null", () => {
    const copy = aBlobStored({ b3: aB3("c"), media: "image/webp" });
    const large = aBlobStored({ b3: aB3("f"), agent_ready: false, meta: { width: 4000, height: 3000, duration_ms: null, triangles: null } });
    const fields = { name: "harbour.png", title: "  ", matters: " The light. ", as: "mira" };
    expect(attachBody("C-0007", { file: large, copy }, fields)).toEqual({ on: "C-0007", b3: aB3("f"), agent_b3: aB3("c"), name: "harbour.png", title: null, matters: "The light.", as: "mira" });
    // Agent-ready as it is: the daemon refuses a copy (400), so none is named.
    expect(attachBody("C-0007", { file: aBlobStored({ b3: aB3("f") }), copy }, fields).agent_b3).toBeNull();
    // Not an image: none.
    expect(attachBody("C-0007", { file: aBlobStored({ b3: aB3("f"), class: "audio", media: "audio/wav", agent_ready: false }), copy }, fields).agent_b3).toBeNull();
    expect(attachBody("C-0007", { file: large, copy: null }, fields).agent_b3).toBeNull();
  });
});
