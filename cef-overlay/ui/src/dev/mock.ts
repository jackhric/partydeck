import type { PdRect } from "../state";

const W = 1280;
const H = 800;

function rects(count: number, shifted: boolean): PdRect[] {
  const dx = shifted ? 24 : 0;
  if (count === 4) {
    return [
      { x: 0, y: 0, w: W / 2 + dx, h: H / 2 },
      { x: W / 2 + dx, y: 0, w: W / 2 - dx, h: H / 2 },
      { x: 0, y: H / 2, w: W / 2 + dx, h: H / 2 },
      { x: W / 2 + dx, y: H / 2, w: W / 2 - dx, h: H / 2 },
    ];
  }
  return [
    { x: 0, y: 0, w: W / 2 + dx, h: H },
    { x: W / 2 + dx, y: 0, w: W / 2 - dx, h: H },
  ];
}

export function start() {
  const count = new URLSearchParams(location.search).get("mock") === "4" ? 4 : 2;
  const t0 = performance.now();

  setInterval(() => {
    const t = performance.now() - t0;
    const slots = rects(count, t >= 5000).map((rect, i) => ({
      rect,
      // Slot 0 flips back to false at 9 s: the cover must NOT re-show.
      live: i === 0 ? t >= 4000 && t < 9000 : t >= 4000 + i * 3000,
      status: null,
      label: `Player ${i + 1}`,
    }));
    window.__pdState?.({ size: { w: W, h: H }, focus: 0, slots });
  }, 200);
}
