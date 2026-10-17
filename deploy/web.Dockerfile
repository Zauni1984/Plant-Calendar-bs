# Web app + reverse proxy (Caddy, automatic HTTPS). Build context: repository root.
#   docker compose build web
#
# The web app is static: built once with the public API address baked in,
# then served by Caddy, which also forwards the API subdomain to the backend.

FROM node:22-bookworm-slim AS build
WORKDIR /app
COPY desktop-ui/package.json desktop-ui/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY desktop-ui ./
ARG VITE_API_BASE_URL
RUN test -n "$VITE_API_BASE_URL" || (echo "VITE_API_BASE_URL is required" && exit 1)
ENV VITE_API_BASE_URL=$VITE_API_BASE_URL
RUN npm run build

FROM caddy:2.10-alpine
COPY deploy/Caddyfile /etc/caddy/Caddyfile
COPY --from=build /app/dist /srv/app
# Only the public legal texts; never docs/INTERNAL.
COPY docs/PRIVACY_POLICY_DE.md docs/PRIVACY_POLICY_EN.md docs/IMPRESSUM_DE.md /srv/app/docs/
