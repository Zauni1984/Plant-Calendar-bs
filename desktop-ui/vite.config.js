import { defineConfig } from 'vite'

export default defineConfig({
  server: { port: 1420, strictPort: true },
  build: {
    // main.js awaits the active locale at top level before the first render.
    // ES2022 covers Safari 15+, Chrome/Edge 89+, Firefox 89+ and the Tauri webviews.
    target: 'es2022',
  },
})
