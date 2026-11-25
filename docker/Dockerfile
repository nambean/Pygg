# Pygg's gateway (pygg-cli, no window) built from the repository and run headless.
# Build:  docker build -f docker/Dockerfile --build-context docker=docker -t pygg:local .
#         (from the repository root; docker/run.sh does it)
# Run:    docker/run.sh up

FROM rust:1-bookworm AS build
WORKDIR /src
COPY . .
WORKDIR /src/apps/desktop/src-tauri
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/apps/desktop/src-tauri/target \
    cargo build --release --locked --no-default-features --bin pygg-cli \
 && cp target/release/pygg-cli /usr/local/bin/pygg

FROM debian:bookworm-slim
# dbus + gnome-keyring: the Secret Service the vault keeps provider keys in (vault.rs).
# socat: the gateway binds 127.0.0.1 only (gateway/mod.rs), so a relay inside the container takes Docker's port.
# git: smart tasks make worktrees.
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates dbus gnome-keyring socat git tini \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --create-home --home-dir /data/home --uid 1000 pygg \
 && mkdir -p /data/store && chown -R pygg:pygg /data
COPY --from=build /usr/local/bin/pygg /usr/local/bin/pygg
COPY --from=docker --chmod=0755 entrypoint.sh /usr/local/bin/pygg-entrypoint

USER pygg
ENV HOME=/data/home \
    PYGG_DATA_DIR=/data/store \
    DBUS_SESSION_BUS_ADDRESS=unix:path=/tmp/pygg-dbus.sock
VOLUME ["/data"]
EXPOSE 8742
ENTRYPOINT ["tini", "--", "pygg-entrypoint"]
CMD ["gateway", "serve", "--port", "8741"]
