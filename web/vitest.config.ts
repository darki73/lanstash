/**
 * Vitest: the mirrored tests under tests/, run in happy-dom.
 */
import { defineConfig, mergeConfig } from "vitest/config"
import vite from "./vite.config.ts"

export default mergeConfig(
  vite,
  defineConfig({
    test: {
      environment: "happy-dom",
      include: ["tests/**/*.test.{ts,tsx}"],
      setupFiles: ["./vitest.setup.ts"],
    },
  }),
)
