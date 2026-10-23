import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [
    tailwindcss(),
    sveltekit({
      preprocess: vitePreprocess(),
      // Tauri doesn't have a Node.js server to do proper SSR
      // so we will use adapter-static to prerender the app (SSG)
      // See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
      adapter: adapter(),
      alias: {
        // SvelteKit 3 dropped the built-in `$lib` alias in favour of `#lib`;
        // keep `$lib` so existing imports and shadcn-svelte generated code work.
        $lib: "src/lib",
        "@/*": "./lib/*",
      },
    }),
  ],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: {
      ignored: ["**/src-tauri/**"]
    }
  }
}));
