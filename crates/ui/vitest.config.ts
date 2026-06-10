import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import solid from "vite-plugin-solid";

const REACT_FILES = /\.react\.[jt]sx$/;
const SOLID_FILES = /\.react\.[jt]sx$/;

export default defineConfig({
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
  resolve: {
    conditions: ["browser"],
  },
  test: {
    environment: "jsdom",
  },
});
