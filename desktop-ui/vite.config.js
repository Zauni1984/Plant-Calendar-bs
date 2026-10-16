import { defineConfig } from 'vite'
import { VitePWA } from 'vite-plugin-pwa'

export default defineConfig({
  server: { port: 1420, strictPort: true },
  preview: { port: 4173, strictPort: true },
  build: {
    // main.js awaits the active locale at top level before the first render.
    // ES2022 covers Safari 15+, Chrome/Edge 89+, Firefox 89+ and the Tauri webviews.
    target: 'es2022',
  },
  plugins: [
    VitePWA({
      // Our own service worker (src/sw.js); the plugin only injects the
      // precache list into it. Push notifications need custom code there.
      strategies: 'injectManifest',
      srcDir: 'src',
      filename: 'sw.js',
      // Registered from main.js, and only outside Tauri.
      injectRegister: false,
      // public/manifest.webmanifest is maintained by hand.
      manifest: false,
      injectManifest: {
        // Only the app shell. Locales and Chart.js are cached on first use.
        globPatterns: [
          'index.html',
          'assets/index-*.{js,css}',
          // Tiny chunks every start needs (lazy-import helper, SW registration).
          'assets/{preload-helper,virtual_pwa-register,workbox-window}*.js',
          'icons/*.{png,svg}',
          'manifest.webmanifest',
        ],
      },
      // A service worker in dev caches stale modules and hides edits.
      devOptions: { enabled: false },
    }),
  ],
})
