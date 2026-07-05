import { useSyncExternalStore } from "react";
import SlotCover from "./SlotCover";
import { pdStore } from "./state";

export default function App() {
  const state = useSyncExternalStore(pdStore.subscribe, pdStore.getSnapshot);
  return (
    <>
      {state.slots.map((slot, i) => (
        <SlotCover key={i} rect={slot.rect} live={slot.live} />
      ))}
    </>
  );
}
