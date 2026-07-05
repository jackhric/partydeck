import "./state";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./overlay.css";

if (import.meta.env.DEV && new URLSearchParams(location.search).has("mock")) {
  import("./dev/mock").then((m) => m.start());
}

createRoot(document.getElementById("root")!).render(<App />);
