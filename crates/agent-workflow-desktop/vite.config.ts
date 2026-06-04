import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import solid from "vite-plugin-solid";

const REACT_FILES = /\.react\.[jt]sx$/;
const SOLID_FILES = /\.react\.[jt]sx$/;

// @ts-expect-error process is provided by the Node runtime
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [
    react({
      include: REACT_FILES,
    }),
    solid({
      exclude: SOLID_FILES,
    }),
  ],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
});
