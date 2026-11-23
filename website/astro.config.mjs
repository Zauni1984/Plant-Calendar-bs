// Marketing site for sowling.com. Static HTML for the Hostinger webspace:
// `npm run build` writes everything to dist/, which is uploaded as is.
import { defineConfig } from "astro/config";

export default defineConfig({
  site: "https://sowling.com",
  trailingSlash: "always",
  build: { format: "directory" },
  compressHTML: true,
  // The legal texts live in ../docs and are shared with the app.
  vite: { server: { fs: { allow: [".."] } } },
});
