import type { PdRect } from "./state";

export interface Edges {
  top: boolean;
  bottom: boolean;
  left: boolean;
  right: boolean;
}

// Which edges of `rect` abut another cell. Two rects share an edge when their
// boundary lines coincide (within EPS) and they overlap along the other axis;
// screen-facing outer edges never count.
export function interiorEdges(rect: PdRect, others: PdRect[]): Edges {
  const EPS = 1;
  const overlapsY = (o: PdRect) => rect.y < o.y + o.h - EPS && o.y < rect.y + rect.h - EPS;
  const overlapsX = (o: PdRect) => rect.x < o.x + o.w - EPS && o.x < rect.x + rect.w - EPS;
  const near = (a: number, b: number) => Math.abs(a - b) < EPS;

  return {
    top: others.some((o) => near(o.y + o.h, rect.y) && overlapsX(o)),
    bottom: others.some((o) => near(o.y, rect.y + rect.h) && overlapsX(o)),
    left: others.some((o) => near(o.x + o.w, rect.x) && overlapsY(o)),
    right: others.some((o) => near(o.x, rect.x + rect.w) && overlapsY(o)),
  };
}
