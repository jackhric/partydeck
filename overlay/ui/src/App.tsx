import { useSyncExternalStore } from "react";
import DisconnectedOverlay from "./DisconnectedOverlay";
import { interiorEdges } from "./layout";
import SlotCover from "./SlotCover";
import { borderColor, pdStore } from "./state";

export default function App() {
  const state = useSyncExternalStore(pdStore.subscribe, pdStore.getSnapshot);
  const rects = state.slots.map((s) => s.rect);
  const color = borderColor(state.border);
  return (
    <>
      {state.slots.map((slot, i) => (
        <SlotCover
          key={i}
          index={i}
          rect={slot.rect}
          live={slot.live}
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
          show={slot.controller_disconnected}
        />
      ))}
      {/* Split lines sit above the covers (z-10) so they show while loading and over the game. */}
      {state.slots.map((slot, i) => {
        const e = interiorEdges(slot.rect, rects.filter((_, j) => j !== i));
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
