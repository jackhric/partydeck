import { useSyncExternalStore } from "react";
import { DEFAULT_FONT } from "../fonts";
import type { PdState } from "../state";
import { rects, runTimeline, TEST_AVATARS } from "./mock";

export interface DevSlot {
  live: boolean;
  status: string;
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
  border: string; // split-line style name: "off" | "faint" | "medium" | "strong"
}

// Cell split-line presets; `value` is the style name pushed in PdState.border.
export const BORDER_PRESETS: { name: string; value: string }[] = [
  { name: "Off", value: "off" },
  { name: "Faint", value: "faint" },
  { name: "Medium", value: "medium" },
  { name: "Strong", value: "strong" },
];

function makeSlot(i: number): DevSlot {
  return {
    live: false,
    status: "",
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
  border: BORDER_PRESETS[1].value, // Faint
};

function applyFont(stack: string) {
  document.documentElement.style.setProperty("--pd-font", stack);
}

const listeners = new Set<() => void>();
let stopTimeline: (() => void) | null = null;

function emit() {
  for (const fn of listeners) fn();
}

// Translate the dev model into the PdState the overlay consumes and push it.
function push() {
  const { w, h, count, focus, slots, border } = model;
  const r = rects(count, w, h);
  window.__pdState?.({
    size: { w, h },
    focus,
    border,
    slots: r.map((rect, i) => {
      const s = slots[i] ?? makeSlot(i);
      return {
        rect,
        live: s.live,
        status: s.status || null,
        label: s.label || null,
        avatar: s.avatar,
        logo: null,
        controller_disconnected: s.controllerDisconnected,
      };
    }),
  });
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
  setBorder(value: string) {
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
    // Clear, then re-push on the NEXT frame. Doing both synchronously lets React
    // batch them into one render, so covers never unmount and the CSS animations
    // don't restart. The rAF gap forces the empty commit (real unmount) first.
    window.__pdState?.({ size: { w: model.w, h: model.h }, focus: model.focus, slots: [] });
    emit();
    requestAnimationFrame(() => push());
  },
  // Run the original scripted loading timeline at the current resolution/count.
  playTimeline() {
    stopTimelineIfRunning();
    window.__pdState?.({ size: { w: model.w, h: model.h }, focus: model.focus, slots: [] });
    model = { ...model, timeline: true };
    stopTimeline = runTimeline(model.count, model.w, model.h, (s: PdState) =>
      window.__pdState?.(s),
    );
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
