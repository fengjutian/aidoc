import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Tauri expects a fixed port + no host header checks during dev.
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
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
    // Manual chunking keeps the initial app shell small so first paint only
    // pulls in React + the editor. Heavy optional libs (mermaid, etc.) get
    // their own vendor chunks and only load when actually used.
    rollupOptions: {
      output: {
        manualChunks: {
          react: ["react", "react-dom"],
          tiptap: [
            "@tiptap/react",
            "@tiptap/starter-kit",
            "@tiptap/extension-link",
            "@tiptap/extension-placeholder",
          ],
          mermaid: ["mermaid"],
        },
      },
    },
    chunkSizeWarningLimit: 800,
  },
});