import { useEffect, useRef, useState } from "react";
import PartydeckLogo from "./PartydeckLogo";
import type { PdRect } from "./state";

type Phase = "loading" | "fading" | "done";

export default function SlotCover({ rect, live }: { rect: PdRect; live: boolean }) {
  const [phase, setPhase] = useState<Phase>("loading");
  // One-shot: once live has been seen true, a later live:false (compositor
  // hiccup) must never re-show the cover or cancel the pending fade.
  const latched = useRef(false);
  const holdTimer = useRef<number>(undefined);

  useEffect(() => {
    if (!live || latched.current) return;
    latched.current = true;
    holdTimer.current = window.setTimeout(() => setPhase("fading"), 1000);
  }, [live]);

  useEffect(() => () => clearTimeout(holdTimer.current), []);

  useEffect(() => {
    if (phase !== "fading") return;
    // Fallback in case transitionend never fires (tab throttling etc.).
    const t = window.setTimeout(() => setPhase("done"), 900);
    return () => clearTimeout(t);
  }, [phase]);

  if (phase === "done") return null;

  return (
    <div
      className={
        "absolute flex items-center justify-center bg-black transition-opacity duration-700 ease-in-out" +
        (phase === "fading" ? " opacity-0" : "")
      }
      style={{ left: rect.x, top: rect.y, width: rect.w, height: rect.h }}
      onTransitionEnd={(e) => {
        if (e.target === e.currentTarget && e.propertyName === "opacity") {
          setPhase("done");
        }
      }}
    >
      <PartydeckLogo />
    </div>
  );
}
