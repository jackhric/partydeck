import "./state";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./fonts";
import "./overlay.css";

// Dev sidebar: on in `vite dev` (so bare / and /overlay.html both land here),
// off in the production single-file build. Opt out with ?nopanel.
if (import.meta.env.DEV && !new URLSearchParams(location.search).has("nopanel")) {
  import("./dev/mount").then((m) => m.start());
}

createRoot(document.getElementById("root")!).render(<App />);
