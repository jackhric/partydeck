import { useState } from "react";
import { FONTS } from "../fonts";
import { TEST_AVATARS } from "./mock";
import { BORDER_PRESETS, devStore, useDevModel } from "./store";

const RES_PRESETS: [string, number, number][] = [
  ["Deck", 1280, 800],
  ["1080p", 1920, 1080],
  ["1600p", 2560, 1600],
];

export default function DevPanel() {
  const m = useDevModel();
  const [collapsed, setCollapsed] = useState(false);
  const [w, setW] = useState(String(m.w));
  const [h, setH] = useState(String(m.h));

  const applyCustom = () => {
    const nw = Math.max(1, parseInt(w, 10) || m.w);
    const nh = Math.max(1, parseInt(h, 10) || m.h);
    setW(String(nw));
    setH(String(nh));
    devStore.setResolution(nw, nh);
  };

  const applyPreset = (pw: number, ph: number) => {
    setW(String(pw));
    setH(String(ph));
    devStore.setResolution(pw, ph);
  };

  if (collapsed) {
    return (
      <button style={{ ...S.reopen }} onClick={() => setCollapsed(false)}>
        ⚙ dev
      </button>
    );
  }

  return (
    <div style={S.panel}>
      <div style={S.header}>
        <span style={{ fontWeight: 700 }}>overlay dev</span>
        <button style={S.iconBtn} onClick={() => setCollapsed(true)} title="Hide">
          ✕
        </button>
      </div>

      <Section title="Resolution">
        <div style={S.row}>
          {RES_PRESETS.map(([label, pw, ph]) => (
            <button
              key={label}
              style={active(m.w === pw && m.h === ph)}
              onClick={() => applyPreset(pw, ph)}
            >
              {label}
            </button>
          ))}
        </div>
        <div style={S.row}>
          <input
            style={S.num}
            value={w}
            onChange={(e) => setW(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && applyCustom()}
          />
          <span style={{ opacity: 0.6 }}>×</span>
          <input
            style={S.num}
            value={h}
            onChange={(e) => setH(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && applyCustom()}
          />
          <button style={S.btn} onClick={applyCustom}>
            Apply
          </button>
        </div>
      </Section>

      <Section title="Layout">
        <div style={S.row}>
          <span style={{ opacity: 0.7, minWidth: 42 }}>Slots</span>
          {[1, 2, 4].map((n) => (
            <button key={n} style={active(m.count === n)} onClick={() => devStore.setCount(n)}>
              {n}
            </button>
          ))}
        </div>
        <div style={S.row}>
          <span style={{ opacity: 0.7, minWidth: 42 }}>Borders</span>
          {BORDER_PRESETS.map((b) => (
            <button
              key={b.name}
              style={active(m.border === b.value)}
              onClick={() => devStore.setBorder(b.value)}
            >
              {b.name}
            </button>
          ))}
        </div>
      </Section>

      <Section title="Player font">
        <div style={S.row}>
          {FONTS.map((f) => (
            <button
              key={f.name}
              style={{ ...active(m.font === f.stack), fontFamily: f.stack }}
              onClick={() => devStore.setFont(f.stack)}
            >
              {f.name}
            </button>
          ))}
        </div>
      </Section>

      <Section title="Slots">
        {m.slots.map((s, i) => (
          <div key={i} style={S.slot}>
            <div style={S.row}>
              <button
                style={active(m.focus === i)}
                onClick={() => devStore.setFocus(i)}
                title="Focus this slot"
              >
                {m.focus === i ? "◉" : "○"}
              </button>
              <strong style={{ minWidth: 42 }}>#{i}</strong>
              <label style={{ ...S.check, marginLeft: "auto" }}>
                <input
                  type="checkbox"
                  checked={s.live}
                  onChange={(e) => devStore.patchSlot(i, { live: e.target.checked })}
                />
                live
              </label>
              <label style={S.check} title="Show controller-disconnected overlay">
                <input
                  type="checkbox"
                  checked={s.controllerDisconnected}
                  onChange={(e) =>
                    devStore.patchSlot(i, { controllerDisconnected: e.target.checked })
                  }
                />
                no pad
              </label>
            </div>
            <input
              style={S.text}
              placeholder="label"
              value={s.label}
              onChange={(e) => devStore.patchSlot(i, { label: e.target.value })}
            />
            <input
              style={S.text}
              placeholder="status"
              value={s.status}
              onChange={(e) => devStore.patchSlot(i, { status: e.target.value })}
            />
            <div style={S.row}>
              <span style={{ opacity: 0.7, minWidth: 42 }}>Icon</span>
              <button
                style={active(!s.avatar)}
                onClick={() => devStore.patchSlot(i, { avatar: null })}
              >
                None
              </button>
              {TEST_AVATARS.map((a) => (
                <button
                  key={a.name}
                  style={active(s.avatar === a.data)}
                  onClick={() => devStore.patchSlot(i, { avatar: a.data })}
                  title={a.name}
                >
                  {a.name}
                </button>
              ))}
            </div>
          </div>
        ))}
      </Section>

      <Section title="Transitions">
        <div style={S.row}>
          <button style={S.btn} onClick={() => devStore.reset()}>
            ⟲ Reset (all loading)
          </button>
        </div>
        <div style={S.row}>
          <button style={active(m.timeline)} onClick={() => devStore.playTimeline()}>
            ▶ Play scripted timeline
          </button>
        </div>
        <p style={S.hint}>
          Toggle <em>live</em> to trigger the 1&nbsp;s hold → fade. Reset re-mounts covers so you
          can replay from <em>loading</em>.
        </p>
      </Section>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div style={S.section}>
      <div style={S.sectionTitle}>{title}</div>
      {children}
    </div>
  );
}

const BASE_BTN: React.CSSProperties = {
  background: "#2a2a33",
  color: "#e8e8ef",
  borderStyle: "solid",
  borderWidth: 1,
  borderColor: "#3a3a45",
  borderRadius: 6,
  padding: "4px 8px",
  fontSize: 12,
  cursor: "pointer",
};

function active(on: boolean): React.CSSProperties {
  return on ? { ...BASE_BTN, background: "#4f7cff", borderColor: "#4f7cff", color: "#fff" } : BASE_BTN;
}

const S: Record<string, React.CSSProperties> = {
  panel: {
    position: "fixed",
    top: 0,
    right: 0,
    width: 260,
    maxHeight: "100vh",
    overflowY: "auto",
    zIndex: 2147483647,
    background: "#17171c",
    color: "#e8e8ef",
    font: "13px/1.4 ui-sans-serif, system-ui, sans-serif",
    boxShadow: "-2px 0 12px rgba(0,0,0,0.5)",
    padding: 12,
    boxSizing: "border-box",
  },
  reopen: {
    ...BASE_BTN,
    position: "fixed",
    top: 8,
    right: 8,
    zIndex: 2147483647,
    background: "#17171c",
  },
  header: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    marginBottom: 8,
  },
  iconBtn: { ...BASE_BTN, padding: "2px 6px" },
  section: { borderTop: "1px solid #2a2a33", padding: "8px 0" },
  sectionTitle: {
    fontSize: 11,
    textTransform: "uppercase",
    letterSpacing: 0.5,
    opacity: 0.55,
    marginBottom: 6,
  },
  row: { display: "flex", alignItems: "center", gap: 6, marginBottom: 6, flexWrap: "wrap" },
  btn: BASE_BTN,
  num: {
    width: 60,
    background: "#0f0f13",
    color: "#e8e8ef",
    border: "1px solid #3a3a45",
    borderRadius: 6,
    padding: "4px 6px",
    fontSize: 12,
  },
  text: {
    width: "100%",
    background: "#0f0f13",
    color: "#e8e8ef",
    border: "1px solid #3a3a45",
    borderRadius: 6,
    padding: "4px 6px",
    fontSize: 12,
    marginBottom: 4,
    boxSizing: "border-box",
  },
  check: { display: "flex", alignItems: "center", gap: 4, fontSize: 12, cursor: "pointer" },
  slot: {
    border: "1px solid #2a2a33",
    borderRadius: 6,
    padding: 6,
    marginBottom: 6,
    background: "#1c1c22",
  },
  hint: { fontSize: 11, opacity: 0.5, margin: "4px 0 0" },
};
