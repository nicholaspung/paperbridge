import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, host: "127.0.0.1" },
  build: { target: "safari15", assetsInlineLimit: 0 },
  ...(process.env.VITEST ? { resolve: { conditions: ["browser"] } } : {}),
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.ts"],
    setupFiles: ["src/test-setup.ts"],
  },
});
