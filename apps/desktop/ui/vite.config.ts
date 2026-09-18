import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed port + no host header checks during dev.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: "127.0.0.1",
    hmr: { host: "127.0.0.1", port: 5173 },
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    target: "es2022",
    sourcemap: true,
  },
});