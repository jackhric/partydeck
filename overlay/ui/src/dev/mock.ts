import type { PdRect, PdSlot, PdState } from "../state";

export const SAMPLE_AVATAR =
  "iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAIAAAD8GO2jAAAAKklEQVR42mOIeVBDU8QwasGoBaMWjFowasGoBaMWjFowasGoBaMWDBULAP1h4Fu1TRxEAAAAAElFTkSuQmCC";

// Distinct two-tone test avatars for exercising per-slot gradient extraction.
// "Gray" has no usable hue and should drive the SteamOS-blue fallback.
export const TEST_AVATARS: { name: string; data: string }[] = [
  {
    name: "Crimson",
    data: "iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAIAAACQkWg2AAAAUUlEQVR4nJXLSQ0AIAwFUeQgohKRiJdCAmHt8pvMcV6qOYdKoZsLBQAXCoBxo2DdEDhvHzy3A/7bAuKtAu2WgXELwL5f4N4XQO4NwHsC/O6gAa2ndLhFPVATAAAAAElFTkSuQmCC",
  },
  {
    name: "Emerald",
    data: "iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAIAAACQkWg2AAAAU0lEQVR4nJXLSw0AIAwEUZSgqWeUVBOaEAMJhG9pt8kc54WYk6vgurmQA3AhB+g3CuYNgf22wXUb4L01IN5f8LtloNwC0O8bmPcBkHsB8B4AvxuoYLpfAKzxpYgAAAAASUVORK5CYII=",
  },
  {
    name: "Violet",
    data: "iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAIAAACQkWg2AAAAU0lEQVR4nJXLSw0AIAwEUdQhAmEoqZye0QEJhG9pt8kc54Uci6vgujmRA3AiB+g3CuYNgf22wXUb4L01IN5f8LtloNwC0O8bmPcBkHsB8B4AvxuoED3U0JgI2qIAAAAASUVORK5CYII=",
  },
  {
    name: "Gray",
    data: "iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAIAAACQkWg2AAAAS0lEQVR4nJXLwQ0AIAgEQcqx/1yBGqNRBOHY9460YlK6ARQAZizAjgK4ygF0CYApAvaOgHt/we/2QXA7IL5fkN4KMPcB5L0Afw/QAQGcPPDMu25rAAAAAElFTkSuQmCC",
  },
];

// Even split of a w×h surface into `count` slot rects.
export function rects(count: number, w: number, h: number): PdRect[] {
  if (count === 4) {
    return [
      { x: 0, y: 0, w: w / 2, h: h / 2 },
      { x: w / 2, y: 0, w: w / 2, h: h / 2 },
      { x: 0, y: h / 2, w: w / 2, h: h / 2 },
      { x: w / 2, y: h / 2, w: w / 2, h: h / 2 },
    ];
  }
  if (count === 1) {
    return [{ x: 0, y: 0, w, h }];
  }
  return [
    { x: 0, y: 0, w: w / 2, h },
    { x: w / 2, y: 0, w: w / 2, h },
  ];
}

// Runs the original scripted loading timeline against an arbitrary resolution,
// pushing frames through the provided sink. Returns a stop() to cancel it.
export function runTimeline(
  count: number,
  w: number,
  h: number,
  sink: (s: PdState) => void,
): () => void {
  const t0 = performance.now();
  const id = window.setInterval(() => {
    const t = performance.now() - t0;
    const slots: PdSlot[] = rects(count, w, h).map((rect, i) => ({
      rect,
      // Slot 0 flips back to false at 9 s: the cover must NOT re-show.
      live: i === 0 ? t >= 4000 && t < 9000 : t >= 4000 + i * 3000,
      status: null,
      label: `Player ${i + 1}`,
      avatar: i === 0 ? SAMPLE_AVATAR : null,
      logo: SAMPLE_AVATAR,
    }));
    sink({ size: { w, h }, focus: 0, slots });
  }, 200);
  return () => clearInterval(id);
}
