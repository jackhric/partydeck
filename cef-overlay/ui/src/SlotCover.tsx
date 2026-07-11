import { AnimatePresence, motion, type Variants } from "motion/react";
import { useEffect, useRef, useState } from "react";
import PartydeckLogo, { LOGO_DUR, LOGO_HOLD, STAGGER } from "./PartydeckLogo";
import type { PdRect } from "./state";

const HOLD_MS = 1000; // dwell on the cover after live before lifting it
const COVER_FADE = 0.7; // cover fade-out duration (seconds)

// The card (icon + name) slides in from the right once the logo has faded in.
const CARD_DELAY = LOGO_HOLD + LOGO_DUR; // start as the logo settles
const CARD_SLIDE = 32; // px the icon/name travel leftward into place

// `custom` carries the per-cell delay so the whole card enters after the logo.
// Icon + name live inside this one motion element so they slide as a single unit
// rather than staggering (which let the icon arrive before the name).
const cardVariants: Variants = {
  hidden: { opacity: 0, x: CARD_SLIDE },
  shown: (delay: number) => ({
    opacity: 1,
    x: 0,
    transition: { duration: 0.5, ease: "easeOut", delay },
  }),
};

const logoVariants: Variants = {
  hidden: { opacity: 0 },
  shown: (delay: number) => ({
    opacity: 0.25,
    transition: { duration: 0.5, ease: "easeOut", delay },
  }),
};

export default function SlotCover({
  index,
  rect,
  live,
  label,
  avatar,
  logo,
}: {
  index: number;
  rect: PdRect;
  live: boolean;
  status: string | null;
  label: string | null;
  avatar: string | null;
  logo: string | null;
}) {
  const [lifted, setLifted] = useState(false);
  // One-shot: once live has been seen true, a later live:false (compositor
  // hiccup) must never re-show the cover or cancel the pending lift.
  const latched = useRef(false);
  const holdTimer = useRef<number>(undefined);

  useEffect(() => {
    if (!live || latched.current) return;
    latched.current = true;
    holdTimer.current = window.setTimeout(() => setLifted(true), HOLD_MS);
  }, [live]);

  useEffect(() => () => clearTimeout(holdTimer.current), []);

  return (
    <AnimatePresence>
      {!lifted && (
        <motion.div
          className="absolute flex flex-col items-center justify-center bg-black"
          style={{ left: rect.x, top: rect.y, width: rect.w, height: rect.h }}
          exit={{ opacity: 0 }}
          transition={{ duration: COVER_FADE, ease: "easeInOut" }}
        >
          <PartydeckLogo avatar={avatar} index={index} />
          {label && (
            <motion.div
              className="mt-16 flex items-center gap-3"
              variants={cardVariants}
              custom={CARD_DELAY + index * STAGGER}
              initial="hidden"
              animate="shown"
            >
              {avatar ? (
                <img
                  src={"data:image/png;base64," + avatar}
                  className="w-8 h-8 shrink-0 aspect-square object-cover rounded"
                />
              ) : (
                <div className="w-8 h-8 shrink-0 aspect-square bg-white/15 rounded" />
              )}
              <span
                className="text-neutral-400 text-[min(4vw,1.5rem)] font-bold"
                style={{ fontFamily: "var(--pd-font, Arial, ui-sans-serif, system-ui, sans-serif)" }}
              >
                {label}
              </span>
            </motion.div>
          )}
          {logo && (
            <motion.img
              src={"data:image/png;base64," + logo}
              className="absolute bottom-[5%] right-[5%]"
              style={{ width: "min(45%,340px)", height: "auto", objectFit: "contain" }}
              variants={logoVariants}
              custom={CARD_DELAY + index * STAGGER + 0.15}
              initial="hidden"
              animate="shown"
            />
          )}
        </motion.div>
      )}
    </AnimatePresence>
  );
}
