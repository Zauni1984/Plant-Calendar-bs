# Backend image. Build context: repository root.
#   docker compose build backend
#
# The workspace also contains the Tauri desktop app, which a server neither
# needs nor can build (GTK/WebKit). It is dropped from the workspace here.

FROM rust:1.94-bookworm AS build
WORKDIR /src

COPY Cargo.toml Cargo.lock* ./
COPY shared ./shared
COPY backend ./backend
COPY migrations ./migrations
# The push and (later) mail texts are compiled in from the app's locale files.
COPY desktop-ui/src/locales ./desktop-ui/src/locales

RUN sed -i '/desktop-ui\/src-tauri/d' Cargo.toml \
 && cargo build --release -p backend \
 && cp target/release/backend /usr/local/bin/sowling-backend

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home-dir /data sowling \
 && mkdir -p /data/photos && chown -R sowling /data
COPY --from=build /usr/local/bin/sowling-backend /usr/local/bin/sowling-backend

USER sowling
ENV PHOTO_STORAGE_DIR=/data/photos
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
  CMD curl -fsS http://127.0.0.1:8080/health || exit 1
ENTRYPOINT ["sowling-backend"]
