/**
 * Vite build of the web front end: the status UI of lanstash. The build (dist/) is meant to be
 * served by lanstash on its admin port. During development (pnpm dev) the API (/api) is proxied
 * to the lanstash given in LANSTASH_API (from the environment or web/.env.local, which is not
 * committed; http://localhost:8081 when unset), so the page shows live data while it is being edited.
 * A lanstash with HTTPS on (`admin_listen_tls`) is given as https://<box>:<port>: its plain port sends
 * other machines there.
 */
import { resolve } from "node:path"
import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig, loadEnv } from "vite"

// `.env.local` is read whatever the mode.
const api = process.env.LANSTASH_API ?? loadEnv("", import.meta.dirname, "LANSTASH_").LANSTASH_API

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: { "@": resolve(import.meta.dirname, "src") },
  },
  server: {
    proxy: {
      // An HTTPS lanstash shows a certificate of its own making: the proxy takes it.
      "/api": { target: api ?? "http://localhost:8081", secure: false },
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
})
