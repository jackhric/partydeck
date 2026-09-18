import { describe, expect, it } from "vitest";
import { interiorEdges } from "./layout";
import type { PdRect } from "./state";

const grid: PdRect[] = [
  { x: 0, y: 0, w: 640, h: 400 },
  { x: 640, y: 0, w: 640, h: 400 },
  { x: 0, y: 400, w: 640, h: 400 },
  { x: 640, y: 400, w: 640, h: 400 },
];

describe("interiorEdges", () => {
  it("marks only the edges shared with another cell in a 2x2 grid", () => {
    const others = (i: number) => grid.filter((_, j) => j !== i);
    expect(interiorEdges(grid[0], others(0))).toEqual({ top: false, bottom: true, left: false, right: true });
    expect(interiorEdges(grid[3], others(3))).toEqual({ top: true, bottom: false, left: true, right: false });
  });

  it("draws nothing for a lone cell or cells that only touch at a corner", () => {
    expect(interiorEdges(grid[0], [])).toEqual({ top: false, bottom: false, left: false, right: false });
    // Diagonal neighbour: boundary lines coincide but there is no overlap along the other axis.
    expect(interiorEdges(grid[0], [grid[3]])).toEqual({ top: false, bottom: false, left: false, right: false });
  });
});
