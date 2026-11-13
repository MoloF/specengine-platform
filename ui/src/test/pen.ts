import type { MapPen } from "../map/draw";

// A pen that records what a drawing does (docs/features/corpus-map.md "Acceptance criteria", CM7,
// CM12): each call with the pen's state at that moment, so a test reads which fill, which alpha and
// which dash a disc or a line was drawn in, and how many calls a redraw costs.

export interface PenCall {
  op: "arc" | "moveTo" | "lineTo" | "fill" | "stroke" | "fillText" | "strokeText" | "fillRect" | "beginPath" | "setTransform" | "drawImage";
  args: readonly (number | string)[];
  fillStyle: string;
  strokeStyle: string;
  globalAlpha: number;
  dash: readonly number[];
  lineWidth: number;
}

export interface RecordingPen extends MapPen {
  calls: PenCall[];
}

/** A fresh recording pen. */
export function recordingPen(): RecordingPen {
  let dash: number[] = [];
  const pen: RecordingPen = {
    calls: [],
    globalAlpha: 1,
    fillStyle: "",
    strokeStyle: "",
    lineWidth: 1,
    lineJoin: "miter",
    font: "",
    textAlign: "start",
    textBaseline: "alphabetic",
    setTransform: (...args) => {
      record("setTransform", args);
    },
    fillRect: (...args) => {
      record("fillRect", args);
    },
    beginPath: () => {
      record("beginPath", []);
    },
    moveTo: (...args) => {
      record("moveTo", args);
    },
    lineTo: (...args) => {
      record("lineTo", args);
    },
    arc: (...args) => {
      record("arc", args);
    },
    fill: () => {
      record("fill", []);
    },
    stroke: () => {
      record("stroke", []);
    },
    setLineDash: (segments) => {
      dash = [...segments];
    },
    fillText: (...args) => {
      record("fillText", args);
    },
    strokeText: (...args) => {
      record("strokeText", args);
    },
    drawImage: (_image, ...args) => {
      record("drawImage", args);
    },
  };
  function record(op: PenCall["op"], args: readonly (number | string)[]) {
    pen.calls.push({
      op,
      args,
      fillStyle: typeof pen.fillStyle === "string" ? pen.fillStyle : "",
      strokeStyle: typeof pen.strokeStyle === "string" ? pen.strokeStyle : "",
      globalAlpha: pen.globalAlpha,
      dash,
      lineWidth: pen.lineWidth,
    });
  }
  return pen;
}

/**
 * The paths a recording drew, each closed by its fill or stroke: the arcs' centres and the lines'
 * ends inside it, with the state it was filled or stroked in.
 */
export interface DrawnPath {
  op: "fill" | "stroke";
  arcs: { x: number; y: number; r: number }[];
  segments: number;
  fillStyle: string;
  strokeStyle: string;
  globalAlpha: number;
  dash: readonly number[];
}

export function pathsOf(calls: readonly PenCall[]): DrawnPath[] {
  const paths: DrawnPath[] = [];
  let arcs: DrawnPath["arcs"] = [];
  let segments = 0;
  for (const call of calls) {
    if (call.op === "beginPath") {
      arcs = [];
      segments = 0;
    } else if (call.op === "arc") {
      arcs.push({ x: Number(call.args[0]), y: Number(call.args[1]), r: Number(call.args[2]) });
    } else if (call.op === "lineTo") {
      segments += 1;
    } else if (call.op === "fill" || call.op === "stroke") {
      paths.push({ op: call.op, arcs, segments, fillStyle: call.fillStyle, strokeStyle: call.strokeStyle, globalAlpha: call.globalAlpha, dash: call.dash });
    }
  }
  return paths;
}

/**
 * A pen per canvas, made on its first `getContext`: the visible canvas's and the offscreen base's
 * apart, so a test reads what each drew (docs/features/corpus-map.md "Acceptance criteria", CM12).
 */
export function pensPerCanvas(): { pens: Map<HTMLCanvasElement, RecordingPen>; getContext: (this: HTMLCanvasElement) => CanvasRenderingContext2D } {
  const pens = new Map<HTMLCanvasElement, RecordingPen>();
  return {
    pens,
    getContext(this: HTMLCanvasElement) {
      let pen = pens.get(this);
      if (pen === undefined) {
        pen = recordingPen();
        pens.set(this, pen);
      }
      return pen as unknown as CanvasRenderingContext2D;
    },
  };
}

/** The pen of the canvas on the page, and the one of the offscreen base. */
export function visibleAndBase(pens: Map<HTMLCanvasElement, RecordingPen>): { visible: RecordingPen | undefined; base: RecordingPen | undefined } {
  let visible: RecordingPen | undefined;
  let base: RecordingPen | undefined;
  for (const [canvas, pen] of pens) {
    if (canvas.isConnected) {
      visible = pen;
    } else {
      base = pen;
    }
  }
  return { visible, base };
}
