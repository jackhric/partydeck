import { createRoot } from "react-dom/client";
import App from "./App";
import { pdStore } from "./state";
import "./overlay.css";

// The C shell injects window.__pdState(<state>) whenever the compositor state changes.
window.__pdState = pdStore.push;

// Dev sidebar and font switcher: on in `vite dev`, off in the production
// single-file build. Opt out with ?nopanel.
if (import.meta.env.DEV && !new URLSearchParams(location.search).has("nopanel")) {
  import("./dev/mount").then((m) => m.start());
}

createRoot(document.getElementById("root")!).render(<App />);
