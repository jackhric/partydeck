import { useSyncExternalStore } from "react";
import type { BorderStyle } from "../generated/proto";
import { BORDER_STYLES, type PdState } from "../state";
import { DEFAULT_FONT } from "./fonts";
import { makeState, rects, runTimeline, TEST_AVATARS } from "./mock";

export interface DevSlot {
  live: boolean;
  label: string;
  avatar: string | null; // base64 PNG, or null for no avatar
  controllerDisconnected: boolean;
}

export interface DevModel {
  w: number;
  h: number;
  count: number;
  focus: number;
  slots: DevSlot[];
  timeline: boolean; // scripted timeline currently driving state
  font: string; // font-family stack applied to player text (dev-only)
  border: BorderStyle;
}

export const BORDER_PRESETS = BORDER_STYLES.map((value) => ({
  name: value[0].toUpperCase() + value.slice(1),
  value,
}));

function makeSlot(i: number): DevSlot {
  return {
    live: false,
    label: `Player ${i + 1}`,
    avatar: TEST_AVATARS[i % TEST_AVATARS.length].data,
    controllerDisconnected: false,
  };
}

let model: DevModel = {
  w: 1280,
  h: 800,
  count: 2,
  focus: 0,
  slots: [makeSlot(0), makeSlot(1)],
  timeline: false,
  font: DEFAULT_FONT.stack,
  border: "faint",
};

function applyFont(stack: string) {
  document.documentElement.style.setProperty("--pd-font", stack);
}

const listeners = new Set<() => void>();
let stopTimeline: (() => void) | null = null;

function emit() {
  for (const fn of listeners) fn();
}

function pushState(state: PdState) {
  window.__pdState?.(state);
}

function emptyState(): PdState {
  return makeState({ w: model.w, h: model.h, focus: model.focus, border: model.border, slots: [] });
}

// Translate the dev model into the PdState the overlay consumes and push it.
function push() {
  const { w, h, count, focus, slots, border } = model;
  pushState(
    makeState({
      w,
      h,
      focus,
      border,
      slots: rects(count, w, h).map((rect, i) => {
        const s = slots[i] ?? makeSlot(i);
        return {
          rect,
          live: s.live,
          status: null,
          label: s.label || null,
          avatar: s.avatar,
          logo: null,
          controller_disconnected: s.controllerDisconnected,
        };
      }),
    }),
  );
}

function set(patch: Partial<DevModel>) {
  stopTimelineIfRunning();
  model = { ...model, ...patch };
  reconcileSlots();
  emit();
  push();
}

// Keep the per-slot array length in step with `count`.
function reconcileSlots() {
  const cur = model.slots;
  const next: DevSlot[] = [];
  for (let i = 0; i < model.count; i++) next.push(cur[i] ?? makeSlot(i));
  model.slots = next;
  if (model.focus >= model.count) model.focus = 0;
}

function stopTimelineIfRunning() {
  if (stopTimeline) {
    stopTimeline();
    stopTimeline = null;
    model = { ...model, timeline: false };
  }
}

export const devStore = {
  subscribe(fn: () => void) {
    listeners.add(fn);
    return () => listeners.delete(fn);
  },
  getSnapshot: () => model,

  setResolution(w: number, h: number) {
    set({ w, h });
  },
  setCount(count: number) {
    set({ count });
  },
  setFocus(focus: number) {
    set({ focus });
  },
  setFont(stack: string) {
    model = { ...model, font: stack };
    applyFont(stack);
    emit();
  },
  setBorder(value: BorderStyle) {
    model = { ...model, border: value };
    emit();
    push();
  },
  patchSlot(i: number, patch: Partial<DevSlot>) {
    stopTimelineIfRunning();
    const slots = model.slots.map((s, j) => (j === i ? { ...s, ...patch } : s));
    model = { ...model, slots };
    emit();
    push();
  },
  // Force every slot back to loading and re-mount covers so the fade-in
  // animations replay.
  reset() {
    stopTimelineIfRunning();
    model = {
      ...model,
      slots: model.slots.map((_, i) => makeSlot(i)),
    };
    // Clear, then re-push on the next frame: pushed synchronously, React
    // batches both into one render and the covers never unmount.
    pushState(emptyState());
    emit();
    requestAnimationFrame(() => push());
  },
  // Run the scripted loading timeline at the current resolution/count.
  playTimeline() {
    stopTimelineIfRunning();
    pushState(emptyState());
    model = { ...model, timeline: true };
    stopTimeline = runTimeline(model.count, model.w, model.h, model.border, pushState);
    emit();
  },

  init() {
    applyFont(model.font);
    push();
  },
};

export function useDevModel() {
  return useSyncExternalStore(devStore.subscribe, devStore.getSnapshot);
}
