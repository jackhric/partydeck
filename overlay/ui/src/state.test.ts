import { describe, expect, it, vi } from "vitest";
import { BORDER_COLORS, borderColor, createPdStore, EMPTY_STATE, type PdState } from "./state";

const sample: PdState = {
  proto: 1,
  size: { w: 1280, h: 800 },
  focus: 0,
  border: "strong",
  slots: [],
};

describe("pdStore", () => {
  it("notifies subscribers on push and stops after unsubscribe", () => {
    const store = createPdStore();
    const listener = vi.fn();
    const unsubscribe = store.subscribe(listener);
    expect(store.getSnapshot()).toBe(EMPTY_STATE);

    store.push(sample);
    expect(listener).toHaveBeenCalledTimes(1);
    expect(store.getSnapshot()).toBe(sample);

    unsubscribe();
    store.push({ ...sample, focus: 1 });
    expect(listener).toHaveBeenCalledTimes(1);
    expect(store.getSnapshot().focus).toBe(1);
  });

  it("maps border styles to colors with a faint fallback", () => {
    expect(borderColor("strong")).toBe(BORDER_COLORS.strong);
    expect(borderColor("off")).toBe("transparent");
    expect(borderColor("no-such-style")).toBe(BORDER_COLORS.faint);
  });
});
