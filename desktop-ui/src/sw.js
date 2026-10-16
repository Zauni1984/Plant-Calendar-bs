// Service worker for the web app (never registered inside the Tauri desktop app).
//
// Caching rules
//   - The app shell (HTML, main JS/CSS, icons, manifest) is precached, so the
//     app opens instantly and offline. The list is injected at build time by
//     vite-plugin-pwa; file names are content-hashed, so a new deploy simply
//     brings a new list and stale entries are cleaned up.
//   - Chunks loaded later (the nine non-German locales, Chart.js) are cached
//     on first use instead, so a first visit does not download ~250 KB the
//     visitor may never need.
//   - API responses are never cached: they are personal data, and the API is
//     a different origin anyway. /api/ and /docs/ are also excluded from the
//     app-shell fallback in case they are served from this origin later.

import { cleanupOutdatedCaches, createHandlerBoundToURL, precacheAndRoute } from "workbox-precaching";
import { NavigationRoute, registerRoute } from "workbox-routing";
import { CacheFirst } from "workbox-strategies";
import { ExpirationPlugin } from "workbox-expiration";

cleanupOutdatedCaches();
precacheAndRoute(self.__WB_MANIFEST);

registerRoute(
  new NavigationRoute(createHandlerBoundToURL("/index.html"), {
    denylist: [/^\/api\//, /^\/docs\//],
  }),
);

registerRoute(
  ({ url, request }) =>
    url.origin === self.location.origin &&
    url.pathname.startsWith("/assets/") &&
    request.destination === "script",
  new CacheFirst({
    cacheName: "lazy-chunks",
    plugins: [new ExpirationPlugin({ maxEntries: 40, maxAgeSeconds: 90 * 24 * 60 * 60 })],
  }),
);

// A new version waits until the page asks it to take over (after the user
// confirmed the "update available" prompt), so a running page never ends up
// with a mix of old and new files.
self.addEventListener("message", (event) => {
  if (event.data?.type === "SKIP_WAITING") self.skipWaiting();
});
