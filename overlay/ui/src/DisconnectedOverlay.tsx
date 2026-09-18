import { AnimatePresence, motion } from "motion/react";
import type { PdRect } from "./state";

const GAMEPAD_PATH =
  "M21.58 16.09l-1.09-7.66C20.21 6.46 18.52 5 16.53 5H7.47C5.48 5 3.79 6.46 3.51 8.43l-1.09 7.66C2.2 17.63 3.39 19 4.94 19c.68 0 1.32-.27 1.8-.75L9 16h6l2.25 2.25c.48.48 1.13.75 1.8.75 1.56 0 2.75-1.37 2.53-2.91zM11 11H9v2H8v-2H6v-1h2V8h1v2h2v1zm4-1c-.55 0-1-.45-1-1s.45-1 1-1 1 .45 1 1-.45 1-1 1zm2 3c-.55 0-1-.45-1-1s.45-1 1-1 1 .45 1 1-.45 1-1 1z";

export default function DisconnectedOverlay({
  rect,
  slot,
  show,
}: {
  rect: PdRect;
  slot: number;
  show: boolean;
}) {
  return (
    <AnimatePresence>
      {show && (
        <motion.div
          className="absolute flex flex-col items-center justify-center gap-4 text-center bg-neutral-700/70 pointer-events-none"
          style={{ left: rect.x, top: rect.y, width: rect.w, height: rect.h }}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.35, ease: "easeInOut" }}
        >
          <svg
            className="w-[min(15%,96px)] text-white/90"
            viewBox="0 0 24 24"
            fill="currentColor"
            aria-hidden
          >
            <path d={GAMEPAD_PATH} />
          </svg>
          <div
            style={{ fontFamily: "var(--pd-font, Arial, ui-sans-serif, system-ui, sans-serif)" }}
          >
            <div className="text-white font-bold text-[min(4vw,1.75rem)]">
              No controller detected
            </div>
            <div className="text-white/80 text-[min(2.5vw,1.125rem)] mt-1">
              Re-connect your controller to slot "{slot + 1}"
            </div>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
