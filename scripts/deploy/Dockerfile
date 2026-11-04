# A hub's image (docs/features/hub-deploy.md "The image"), built by
# .github/workflows/image.yml on a version tag, never on the host:
#   docker build -f scripts/deploy/Dockerfile .
# from the repository root, with Dockerfile.dockerignore beside this file.
# Three stages: the web UI, the two binaries with that UI compiled in, and a
# Debian slim runtime holding `spec`, `specengine-http`, git, ssh and the
# three scripts alone.
# Every base is an exact tag and its multi-arch index digest, pending the
# owner (Q7).

# Node 24 with pnpm through corepack, at ui/package.json's `packageManager`.
FROM node:24.22.2-trixie-slim@sha256:54f84b4cf8432b21aaffd5d3ce8d41e12cea0d10d33c09d2f3c9879d49d6b446 AS ui
ENV COREPACK_ENABLE_DOWNLOAD_PROMPT=0
WORKDIR /src/ui
COPY ui/package.json ui/pnpm-lock.yaml ui/pnpm-workspace.yaml ui/.npmrc ./
RUN corepack enable && pnpm install --frozen-lockfile
COPY ui/ ./
RUN pnpm build

# The workspace's `rust-version`, 1.90; `ui/dist` in place before cargo runs, so
# the page is compiled into specengine-http.
FROM rust:1.90.0-slim-trixie@sha256:7fa728f3678acf5980d5db70960cf8491aff9411976789086676bdf0c19db39e AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY --from=ui /src/ui/dist ui/dist
RUN cargo build --locked --release -p specengine-cli -p specengine-http

# The runtime: git 2.38 or later (`merge-tree --write-tree`), ssh for the deploy
# key, the CA roots, and the base's `sh`, which a sync's SIGTERM needs. Nothing
# more: no git extension that fetches content on checkout (hub-git.md "Known
# limits"), no Rust toolchain, no rust-analyzer.
FROM debian:trixie-20261027-slim@sha256:f6b08b09a05725f9431502d3b5714f5c4576e82c1a84fa1debe0c24a7e0c3ec4
# GHCR links the package to the repository it names.
LABEL org.opencontainers.image.source=https://github.com/MoloF/specengine-platform
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates git openssh-client \
    && rm -rf /var/lib/apt/lists/*
# uid and gid 10001 own /data and /projects, so a fresh named volume takes
# that ownership, and the user's home, a system path under /var/lib.
RUN groupadd --gid 10001 specengine \
    && useradd --uid 10001 --gid 10001 --no-create-home \
        --home-dir /var/lib/specengine/home --shell /usr/sbin/nologin specengine \
    && mkdir -p /var/lib/specengine/home /data /projects \
    && chown 10001:10001 /var/lib/specengine/home /data /projects
COPY --from=build /src/target/release/spec /src/target/release/specengine-http /usr/local/bin/
COPY --chmod=0755 scripts/deploy/entrypoint.sh /usr/local/bin/specengine-entrypoint
COPY --chmod=0755 scripts/deploy/clone.sh /usr/local/bin/specengine-clone
# The daemon's probe (docs/features/hub-ops.md "`specengine-health`"): bash's
# /dev/tcp, Debian's bash being Essential, so no package and no HTTP client.
# Compose holds the probe's timing; the image declares none.
COPY --chmod=0755 scripts/deploy/health.sh /usr/local/bin/specengine-health
# The data directory lies outside every root.
ENV XDG_DATA_HOME=/data HOME=/var/lib/specengine/home
USER 10001:10001
ENTRYPOINT ["specengine-entrypoint"]
