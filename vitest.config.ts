import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import path from "path";

export default defineConfig({
  // The Svelte plugin compiles `*.svelte.ts` rune modules (e.g. `i18n.svelte.ts`)
  // so pure-TS tests can import code that uses `$state`.
  plugins: [svelte()],
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
  resolve: {
    alias: {
      $lib: path.resolve("./src/lib"),
    },
  },
});
