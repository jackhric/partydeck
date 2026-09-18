import { describe, expect, it } from "vitest";
import { dominant, type Rgb } from "./avatarColors";

function pixels(entries: [Rgb, number, number?][]): Uint8ClampedArray {
  const out: number[] = [];
  for (const [[r, g, b], n, a = 255] of entries) {
    for (let i = 0; i < n; i++) out.push(r, g, b, a);
  }
  return new Uint8ClampedArray(out);
}

describe("dominant", () => {
  it("keeps the most common hues, skips gray and transparent pixels, and orders by lightness", () => {
    const data = pixels([
      [[200, 30, 30], 10], // red, most common, lightness 0.45
      [[30, 30, 120], 6], // dark blue, second, lightness 0.29
      [[30, 200, 30], 2], // green, least common: dropped by count
      [[128, 128, 128], 20], // gray: no hue, ignored even though it is the most frequent
      [[255, 0, 0], 20, 0], // transparent: ignored
    ]);
    expect(dominant(data, 2)).toEqual([
      [30, 30, 120],
      [200, 30, 30],
    ]);
    expect(dominant(pixels([[[128, 128, 128], 50]]), 3)).toEqual([]);
  });
});
