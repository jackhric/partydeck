import { resolve } from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import { viteSingleFile } from "vite-plugin-singlefile";

export default defineConfig({
  plugins: [tailwindcss(), react(), viteSingleFile()],
  build: {
    target: "chrome126",
    outDir: "dist",
    rollupOptions: {
      input: resolve(import.meta.dirname, "overlay.html"),
    },
  },
});
