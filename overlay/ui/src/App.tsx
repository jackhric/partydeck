import { useSyncExternalStore } from "react";
import DisconnectedOverlay from "./DisconnectedOverlay";
import SlotCover from "./SlotCover";
import { borderColor, pdStore } from "./state";
import type { PdRect } from "./state";

// An edge is drawn only where it abuts another cell (interior split line), never
// on the screen-facing outer edge. Two rects share an edge when their boundary
// lines coincide and they overlap along the perpendicular axis.
function interiorEdges(rect: PdRect, others: PdRect[]) {
  const EPS = 1;
  const overlapsY = (o: PdRect) =>
    rect.y < o.y + o.h - EPS && o.y < rect.y + rect.h - EPS;
  const overlapsX = (o: PdRect) =>
    rect.x < o.x + o.w - EPS && o.x < rect.x + rect.w - EPS;
  const near = (a: number, b: number) => Math.abs(a - b) < EPS;

  return {
    top: others.some((o) => near(o.y + o.h, rect.y) && overlapsX(o)),
    bottom: others.some((o) => near(o.y, rect.y + rect.h) && overlapsX(o)),
    left: others.some((o) => near(o.x + o.w, rect.x) && overlapsY(o)),
    right: others.some((o) => near(o.x, rect.x + rect.w) && overlapsY(o)),
  };
}

export default function App() {
  const state = useSyncExternalStore(pdStore.subscribe, pdStore.getSnapshot);
  const rects = state.slots.map((s) => s.rect);
  return (
    <>
      {state.slots.map((slot, i) => (
        <SlotCover
          key={i}
          index={i}
          rect={slot.rect}
          live={slot.live}
          status={slot.status}
          label={slot.label}
          avatar={slot.avatar}
          logo={slot.logo}
        />
      ))}
      {state.slots.map((slot, i) => (
        <DisconnectedOverlay
          key={`dc-${i}`}
          slot={i}
          rect={slot.rect}
          show={slot.controller_disconnected === true}
        />
      ))}
      {/* Split-screen guide lines, drawn persistently ABOVE the covers (z-10) so
          they show during loading and over the game. Only interior edges (those
          touching another cell) get a line — no outer frame. Color derives from
          the config-driven border style in state (default faint). */}
      {state.slots.map((slot, i) => {
        const e = interiorEdges(
          slot.rect,
          rects.filter((_, j) => j !== i),
        );
        const color = borderColor(state.border);
        return (
          <div
            key={`border-${i}`}
            className="absolute z-10 pointer-events-none"
            style={{
              left: slot.rect.x,
              top: slot.rect.y,
              width: slot.rect.w,
              height: slot.rect.h,
              borderStyle: "solid",
              borderWidth: `${e.top ? 1 : 0}px ${e.right ? 1 : 0}px ${e.bottom ? 1 : 0}px ${e.left ? 1 : 0}px`,
              borderColor: color,
            }}
          />
        );
      })}
    </>
  );
}
