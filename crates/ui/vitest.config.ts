import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import solid from "vite-plugin-solid";

const REACT_FILES = /\.react\.[jt]sx$/;
const SOLID_FILES = /\.react\.[jt]sx$/;
const uiRoot = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  resolve: {
    alias: {
      "@": path.resolve(uiRoot, "src"),
    },
    conditions: ["browser"],
  },
  plugins: [
    react({
      include: REACT_FILES,
    }),
    solid({
      exclude: SOLID_FILES,
    }),
  ],
  optimizeDeps: {
    include: ["remark-parse", "remark-rehype", "unified"],
  },
  test: {
    environment: "jsdom",
  },
});
