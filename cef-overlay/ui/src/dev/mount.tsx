import { createRoot } from "react-dom/client";
import DevPanel from "./DevPanel";
import { devStore } from "./store";

// Mount the dev sidebar into its own container so it lives outside #root and
// the overlay's transparent-surface tree. DEV-only; never in the prod bundle.
export function start() {
  devStore.init();
  const host = document.createElement("div");
  host.id = "pd-dev-panel";
  document.body.appendChild(host);
  createRoot(host).render(<DevPanel />);
}
