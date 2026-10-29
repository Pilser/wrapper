import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// /api (and both admin WS feeds) proxy to the wrapper running on
// apiai.pilserlabs.com so the browser never deals with CORS in dev.
// ws: true lets /api/admin/stream and /api/admin/events upgrade cleanly.
export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      "/api": {
        target: "https://apiai.pilserlabs.com",
        changeOrigin: true,
        secure: true,
        ws: true,
      },
    },
  },
});
