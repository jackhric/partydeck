export type Rgb = [number, number, number];

// SteamOS blue gradient — used whenever there's no avatar or extraction fails.
export const STEAMOS_STOPS: Rgb[] = [
  [0x1b, 0x28, 0x38],
  [0x2a, 0x47, 0x5e],
  [0x66, 0xc0, 0xf4],
];

const SAMPLE = 32; // downscale avatars to 32×32 before quantizing

function toHsl([r, g, b]: Rgb): [number, number, number] {
  r /= 255;
  g /= 255;
  b /= 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, l];
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  let h = 0;
  if (max === r) h = (g - b) / d + (g < b ? 6 : 0);
  else if (max === g) h = (b - r) / d + 2;
  else h = (r - g) / d + 4;
  return [h / 6, s, l];
}

function fromHsl(h: number, s: number, l: number): Rgb {
  if (s === 0) {
    const v = Math.round(l * 255);
    return [v, v, v];
  }
  const hue2rgb = (p: number, q: number, t: number) => {
    if (t < 0) t += 1;
    if (t > 1) t -= 1;
    if (t < 1 / 6) return p + (q - p) * 6 * t;
    if (t < 1 / 2) return q;
    if (t < 2 / 3) return p + (q - p) * (2 / 3 - t) * 6;
    return p;
  };
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
  const p = 2 * l - q;
  return [
    Math.round(hue2rgb(p, q, h + 1 / 3) * 255),
    Math.round(hue2rgb(p, q, h) * 255),
    Math.round(hue2rgb(p, q, h - 1 / 3) * 255),
  ];
}

// Boost saturation and pull lightness toward mid so muted/dark avatars still
// yield a gradient with visible pop.
function vibrant(rgb: Rgb): Rgb {
  const [h, s, l] = toHsl(rgb);
  return fromHsl(h, Math.min(1, s * 1.4 + 0.15), Math.min(0.72, Math.max(0.4, l)));
}

// Bucket pixels by a coarse HSL hue/lightness key and return the most common
// buckets' average colors, ordered by lightness for a smooth vertical ramp.
function dominant(pixels: Uint8ClampedArray, count: number): Rgb[] {
  const buckets = new Map<number, { sum: Rgb; n: number }>();
  for (let i = 0; i < pixels.length; i += 4) {
    const a = pixels[i + 3];
    if (a < 128) continue; // skip transparent
    const rgb: Rgb = [pixels[i], pixels[i + 1], pixels[i + 2]];
    const [h, s, l] = toHsl(rgb);
    // Drop near-gray and near-black/white so we key on actual hues.
    if (s < 0.12 || l < 0.08 || l > 0.95) continue;
    const key = (Math.round(h * 11) << 4) | Math.round(l * 4);
    const b = buckets.get(key);
    if (b) {
      b.sum[0] += rgb[0];
      b.sum[1] += rgb[1];
      b.sum[2] += rgb[2];
      b.n++;
    } else {
      buckets.set(key, { sum: [...rgb], n: 1 });
    }
  }
  const sorted = [...buckets.values()].sort((a, b) => b.n - a.n).slice(0, count);
  const colors = sorted.map(
    (b) => [b.sum[0] / b.n, b.sum[1] / b.n, b.sum[2] / b.n].map(Math.round) as Rgb,
  );
  colors.sort((a, b) => toHsl(a)[2] - toHsl(b)[2]);
  return colors;
}

// Decode a base64 PNG and return ~3 vibrant dominant colors. Resolves to the
// SteamOS fallback if the image has no usable hues (grayscale avatar) or fails.
export function extractStops(base64: string): Promise<Rgb[]> {
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => {
      try {
        const canvas = document.createElement("canvas");
        canvas.width = SAMPLE;
        canvas.height = SAMPLE;
        const ctx = canvas.getContext("2d", { willReadFrequently: true });
        if (!ctx) return resolve(STEAMOS_STOPS);
        ctx.drawImage(img, 0, 0, SAMPLE, SAMPLE);
        const data = ctx.getImageData(0, 0, SAMPLE, SAMPLE).data;
        const cols = dominant(data, 3);
        if (cols.length < 2) return resolve(STEAMOS_STOPS);
        resolve(cols.map(vibrant));
      } catch {
        resolve(STEAMOS_STOPS);
      }
    };
    img.onerror = () => resolve(STEAMOS_STOPS);
    img.src = "data:image/png;base64," + base64;
  });
}

export function rgbToHex([r, g, b]: Rgb): string {
  return "#" + [r, g, b].map((v) => v.toString(16).padStart(2, "0")).join("");
}
