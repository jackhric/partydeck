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
}

export interface PdState {
  size: { w: number; h: number };
  focus: number;
  slots: PdSlot[];
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
