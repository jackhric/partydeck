import { motion } from "motion/react";
import { useEffect, useId, useRef, useState } from "react";
import { STEAMOS_STOPS, extractStops, rgbToHex, type Rgb } from "./avatarColors";

// Fade sequence (seconds): black hold → logo fades in → gradient blooms over it.
// Each cell is offset by STAGGER * index so the covers cascade.
export const LOGO_HOLD = 0.5;
export const LOGO_DUR = 0.9;
export const GRADIENT_DUR = 1.0;
export const STAGGER = 0.25;
const GRADIENT_DELAY = LOGO_HOLD + LOGO_DUR; // gradient starts as the logo settles

const BOTTOM_RIGHT =
  "m 40.584236,125.77679 c -0.445764,-0.10842 -3.371049,-0.93569 -4.482264,-4.20049 -1.351072,-3.96952 1.245631,-10.0798 6.835434,-12.8915 l 1.74292,-0.8767 5.08493,-0.004 c 6.46449,-0.005 7.51394,0.96642 5.76458,5.33852 l -0.42644,1.06578 -2.03654,0.14669 c -7.19398,0.51819 -9.27826,8.12909 -2.22619,8.12909 5.917107,-0.36778 5.900723,-2.95804 7.17076,-7.22072 0.46219,-1.52223 0.68199,-2.33475 1.80431,-6.66991 0.57721,-2.22959 1.3696,-5.07375 1.81587,-6.5178 0.20841,-0.67437 0.37892,-1.36693 0.37892,-1.53901 0,-2.081626 3.11454,-3.446716 7.87136,-3.449976 l 3.77031,-0.003 -0.09881,0.814187 c -0.10813,0.890952 -1.017837,5.257249 -1.583697,6.594139 -0.85328,2.01591 -2.60579,3.17171 -4.81856,3.17791 -2.16198,0.006 -2.99481,0.28439 -3.55289,1.18739 -1.03156,1.6691 -0.50135,1.9847 3.33429,1.9847 3.60112,0 3.63802,0.0345 2.79893,2.61833 -0.31209,0.96101 -0.80861,2.58073 -1.10338,3.59938 -1.24796,4.31268 -5.00222,7.81536 -9.39567,8.76603 -2.22592,0.48165 -16.62187,0.44369 -18.64817,-0.0492 z";

const TOP_LEFT =
  "M 24.70456,115.40628 c 4.12e-4,-0.68025 1.264137,-5.68234 1.697798,-6.72024 0.817238,-1.95593 2.642535,-3.13549 4.851975,-3.13549 1.793175,0 3.097165,-0.4813 3.597151,-1.32771 0.903459,-1.52943 0.31513,-1.84729 -3.419228,-1.84729 h -3.023809 v -0.98334 c 0,-0.54084 0.164098,-1.463566 0.364662,-2.050516 0.200565,-0.58695 0.488975,-1.54343 0.640911,-2.12551 1.483505,-5.683445 5.55522,-8.153138 6.649722,-8.72605 5.101151,-2.67019 25.146484,-1.8475 25.152524,1.0323 2.8e-4,0.13406 2.080899,10.895456 -5.77546,14.999946 -2.063189,1.37141 -4.47155,0.88942 -6.84959,0.89129 -6.474628,0.005 -7.526256,-0.96262 -5.77547,-5.31667 l 0.43734,-1.087626 2.03653,-0.1467 c 4.44924,-0.32048 7.15524,-3.08349 6.23074,-6.362 -0.7136,-2.53057 -6.355,-2.48103 -8.84336,0.0777 -1.234978,1.26988 -1.301666,1.43226 -2.502264,6.09276 -0.832161,3.230286 -1.930527,7.358366 -2.323141,8.731246 -0.228888,0.80036 -0.679367,2.40771 -1.001066,3.57187 -1.230963,4.45463 -1.952074,4.89479 -8.019044,4.89479 -3.754385,0 -4.127176,-0.0418 -4.126921,-0.46302 z";

// Center of the P's counter (the hole), as a % of the logo box — the radial
// bloom radiates from here. (Whole-logo viewBox is 48.95 × 39.30; the hole sits
// at roughly x≈46%, y≈20% of that box.)
const HOLE_X_PCT = 46;
const HOLE_Y_PCT = 20;

const BLOOM_DUR = 6000; // ms per full rotation — one seamless cycle

// Whole-logo viewBox, and the translate that maps the raw path coords into it.
const VB_W = 48.947636;
const VB_H = 39.299686;

// Seamless bloom via a conic-gradient rotated about the hole. A conic is angular,
// so it loops with ZERO seam by nature: 0°≡360°, and rotating it forever has no
// boundary to snap at (unlike a radial's radius wrap). The palette is laid as a
// cyclic ramp around the circle (last color == first so the 360° join is smooth),
// then the whole thing is spun by `phase` (0→1 = one full turn = identical frame).
function bloomGradient(colors: Rgb[], phase: number): string {
  const seq = [...colors, colors[0]]; // cyclic: closes the 360° seam
  const per = seq.length - 1;
  const list = seq
    .map((c, i) => `${rgbToHex(c)} ${((i / per) * 360).toFixed(2)}deg`)
    .join(", ");
  const angle = (phase * 360).toFixed(2);
  return `conic-gradient(from ${angle}deg at ${HOLE_X_PCT}% ${HOLE_Y_PCT}%, ${list})`;
}

export default function PartydeckLogo({
  avatar,
  index = 0,
}: {
  avatar?: string | null;
  index?: number;
}) {
  const gradId = useId().replace(/:/g, "");
  const [colors, setColors] = useState<Rgb[]>(STEAMOS_STOPS);
  const [phase, setPhase] = useState(0);

  useEffect(() => {
    if (!avatar) {
      setColors(STEAMOS_STOPS);
      return;
    }
    let alive = true;
    extractStops(avatar).then((c) => {
      if (alive) setColors(c);
    });
    return () => {
      alive = false;
    };
  }, [avatar]);

  // Drive the outward bloom: phase sweeps 0→1 (one ring) each BLOOM_DUR, wrapping
  // seamlessly. Timer-based (not frame-count) so it's independent of paint rate.
  const startRef = useRef<number | null>(null);
  useEffect(() => {
    let raf = 0;
    const tick = (t: number) => {
      if (startRef.current === null) startRef.current = t;
      setPhase(((t - startRef.current) / BLOOM_DUR) % 1);
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, []);

  // Black hold before this cell fades, plus a per-cell offset so covers cascade.
  const delay = LOGO_HOLD + index * STAGGER;

  return (
    <motion.svg
      className="w-[min(30%,220px)]"
      viewBox="0 0 48.947636 39.299686"
      style={{ willChange: "transform, opacity" }}
      initial={{ opacity: 0, scale: 0.97 }}
      animate={{ opacity: 1, scale: 1 }}
      transition={{
        // Per-value transitions replace the parent config, so the black-hold +
        // stagger delay must live inside EACH one — a top-level `delay` is
        // ignored once opacity/scale have their own transition objects.
        opacity: { duration: LOGO_DUR, delay, ease: "easeOut" },
        // Gentle, non-bouncy spring for the scale so it eases in smoothly with
        // no velocity snap; high damping keeps it from overshooting past 1.0.
        scale: { type: "spring", stiffness: 90, damping: 20, delay },
      }}
    >
      <defs>
        {/* Clip the gradient to the P glyph in SVG user space — vector-exact, so
            it registers pixel-perfectly with the white companion path (a raster
            mask-image, stretched to the box, sat slightly inset and left a thin
            white halo). */}
        <clipPath id={`${gradId}-clip`} clipPathUnits="userSpaceOnUse">
          <path transform="translate(-24.70456,-86.867724)" d={TOP_LEFT} />
        </clipPath>
      </defs>
      {/* White base: the companion half, plus the P itself so it's visible while
          the gradient fades in. The white P carries a thin same-space stroke in
          the background color, painted OVER its fill, which erodes its edge
          UNIFORMLY inward by strokeWidth/2. That tucks the white just inside the
          gradient's clip edge everywhere, so the gradient's anti-aliased edge
          always overhangs it and no white AA fringe peeks out. (A scale-inset
          would erode unevenly — near-zero at the hole, largest at the rim.) */}
      <g transform="translate(-24.70456,-86.867724)">
        <path fill="#ffffff" d={BOTTOM_RIGHT} />
        <path
          fill="#ffffff"
          d={TOP_LEFT}
          stroke="#000"
          strokeWidth={0.15}
          strokeLinejoin="round"
        />
      </g>
      {/* Gradient P: a foreignObject div painted with the animated CSS conic
          gradient, clipped to the P shape. SVG fills can't take a CSS gradient,
          so the bloom lives on an HTML element clipped to the glyph. */}
      <foreignObject
        x={0}
        y={0}
        width={VB_W}
        height={VB_H}
        clipPath={`url(#${gradId}-clip)`}
      >
        <motion.div
          style={{
            width: "100%",
            height: "100%",
            background: bloomGradient(colors, phase),
          }}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{
            duration: GRADIENT_DUR,
            delay: GRADIENT_DELAY + index * STAGGER,
            ease: "easeOut",
          }}
        />
      </foreignObject>
    </motion.svg>
  );
}
