export interface PdRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface PdSlot {
  rect: PdRect;
  live: boolean;
  status: string | null;
  label: string | null;
  avatar: string | null;
}

export interface PdState {
  size: { w: number; h: number };
  focus: number;
  border?: string; // split-line style: "off" | "faint" | "medium" | "strong"
  slots: PdSlot[];
}

// Split-line style name -> CSS color. Unknown/missing falls back to faint.
export const BORDER_COLORS: Record<string, string> = {
  off: "transparent",
  faint: "rgba(255,255,255,0.1)",
  medium: "rgba(255,255,255,0.25)",
  strong: "rgba(255,255,255,0.5)",
};

export function borderColor(style: string | undefined): string {
  return BORDER_COLORS[style ?? "faint"] ?? BORDER_COLORS.faint;
}

declare global {
  interface Window {
    __pdState?: (state: PdState) => void;
  }
}

let current: PdState = { size: { w: 0, h: 0 }, focus: 0, slots: [] };
const listeners = new Set<() => void>();

window.__pdState = (state) => {
  current = state;
  for (const fn of listeners) fn();
};

export const pdStore = {
  subscribe(listener: () => void) {
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  },
  getSnapshot: () => current,
};
