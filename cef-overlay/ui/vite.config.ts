import { resolve } from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig, type Plugin } from "vite";
import { viteSingleFile } from "vite-plugin-singlefile";

// Serve overlay.html at bare "/" during dev so http://localhost:5173/ lands in
// the overlay + dev panel with no query string. Dev-only: excluded from build.
function devRoot(): Plugin {
  return {
    name: "pd-dev-root",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use((req, _res, next) => {
        if (req.url === "/" || req.url?.startsWith("/?")) {
          req.url = "/overlay.html" + (req.url.slice(1) || "");
        }
        next();
      });
    },
  };
}

export default defineConfig({
  plugins: [devRoot(), tailwindcss(), react(), viteSingleFile()],
  build: {
    target: "chrome126",
    outDir: "dist",
    rollupOptions: {
      input: resolve(import.meta.dirname, "overlay.html"),
    },
  },
});
