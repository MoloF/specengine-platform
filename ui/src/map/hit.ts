// Which point a pointer is over (docs/features/corpus-map.md "Acceptance criteria", CM7): a uniform
// grid of the laid-out points, built once per layout, so a hover looks at a few cells, never every
// point. Only a kept point is hit; the nearest whose disc holds the pointer wins, ties to the
// earlier point.

/** A grid cell's side in map units: the largest disc's diameter. */
export const HIT_CELL = 36;

export interface HitGrid {
  cell: number;
  cells: Map<number, number[]>;
}

function keyOf(cx: number, cy: number): number {
  return (cx + 32768) * 65536 + (cy + 32768);
}

/** The grid of `count` points at `positions` (x then y each). */
export function gridOf(positions: Float64Array, count: number, cell: number = HIT_CELL): HitGrid {
  const cells = new Map<number, number[]>();
  for (let point = 0; point < count; point += 1) {
    const key = keyOf(Math.floor((positions[point * 2] ?? 0) / cell), Math.floor((positions[point * 2 + 1] ?? 0) / cell));
    const held = cells.get(key);
    if (held === undefined) {
      cells.set(key, [point]);
    } else {
      held.push(point);
    }
  }
  return { cell, cells };
}

/** The kept point whose disc, widened by `slop`, holds (x, y) in map units, nearest first; -1 when none. */
export function hitAt(grid: HitGrid, positions: Float64Array, radii: Float64Array, kept: Uint8Array, x: number, y: number, slop = 0): number {
  const cx = Math.floor(x / grid.cell);
  const cy = Math.floor(y / grid.cell);
  let best = -1;
  let bestDistance = Infinity;
  for (let dx = -1; dx <= 1; dx += 1) {
    for (let dy = -1; dy <= 1; dy += 1) {
      for (const point of grid.cells.get(keyOf(cx + dx, cy + dy)) ?? []) {
        if (kept[point] !== 1) {
          continue;
        }
        const distance = Math.hypot((positions[point * 2] ?? 0) - x, (positions[point * 2 + 1] ?? 0) - y);
        if (distance <= (radii[point] ?? 0) + slop && (distance < bestDistance || (distance === bestDistance && point < best))) {
          best = point;
          bestDistance = distance;
        }
      }
    }
  }
  return best;
}
