import type { BorderStyle, PixelRect, SlotState, State } from "./generated/proto";

export type { State as PdState, SlotState as PdSlot, PixelRect as PdRect };

export const PROTO_VERSION = 1;

export const EMPTY_STATE: State = {
  proto: PROTO_VERSION,
  size: { w: 0, h: 0 },
  focus: 0,
  border: "faint",
  slots: [],
};

export const BORDER_COLORS: Record<BorderStyle, string> = {
  off: "transparent",
  faint: "rgba(255,255,255,0.1)",
  medium: "rgba(255,255,255,0.25)",
  strong: "rgba(255,255,255,0.5)",
};
export const BORDER_STYLES = Object.keys(BORDER_COLORS) as BorderStyle[];

// Accepts any string so a newer compositor style degrades to faint instead of throwing.
export function borderColor(style: BorderStyle | string): string {
  return BORDER_COLORS[style as BorderStyle] ?? BORDER_COLORS.faint;
}

declare global {
  interface Window {
    __pdState?: (state: State) => void;
  }
}

// useSyncExternalStore-shaped store. `push` is what the C shell calls through
// window.__pdState; the shell only calls it when the JSON changed, so no dedup here.
export function createPdStore(initial: State = EMPTY_STATE) {
  let current = initial;
  const listeners = new Set<() => void>();
  return {
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    getSnapshot: () => current,
    push(state: State) {
      current = state;
      for (const fn of listeners) fn();
    },
  };
}

export const pdStore = createPdStore();
