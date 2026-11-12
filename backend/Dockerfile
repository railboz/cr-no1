# Créno relay — production image.
#   build : docker build -t creno-relay backend
#   run   : docker run --rm -p 127.0.0.1:8080:8080 creno-relay
#
# The release binary is linked statically against musl, so the runtime stage can
# be Alpine instead of debian:bookworm-slim (28 MB compressed, ~78 MB unpacked):
# the final image is ~9 MB unpacked. aws-lc-rs, pulled in by reqwest 0.13 +
# rustls, is C code: cmake and a C toolchain are needed in the builder stage
# only and never ship.
#
# Base images pinned by exact tag, verified on the build host with
# `docker manifest inspect`:
#   rust:1.99-bookworm  -> rustc/cargo 1.99.1, Debian bookworm
#   alpine:3.24         -> Alpine 3.24.4

FROM rust:1.99-bookworm AS builder

# cmake: aws-lc-sys build script. musl-tools: musl-gcc + static libc for the
# musl target (crt-static is the default for that target).
RUN apt-get update \
 && apt-get install -y --no-install-recommends cmake musl-tools \
 && rm -rf /var/lib/apt/lists/*

RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /build
ENV CARGO_TERM_COLOR=never \
    CARGO_NET_RETRY=10 \
    CARGO_INCREMENTAL=0

# Layer 1 — dependencies only. A dummy src/ (same targets as the real crate)
# compiles every dependency once; this layer survives any src/ change.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src \
 && printf 'fn main() {}\n' > src/main.rs \
 && : > src/lib.rs \
 && cargo build --release --locked --target x86_64-unknown-linux-musl \
 && rm -rf src

# Layer 2 — the crate itself: only creno-relay is recompiled.
# COPY keeps the source mtimes of the context, which can be older than the
# dummy artefacts built above: touch every file or cargo considers the crate
# fresh, skips the rebuild and silently ships the 385 kB `fn main() {}` stub
# (image that starts, exits 0 and never listens). The size guard turns any
# recurrence into a build failure instead of a broken image.
COPY src ./src
RUN find src -type f -exec touch {} + \
 && cargo build --release --locked --target x86_64-unknown-linux-musl \
 && test "$(stat -c %s target/x86_64-unknown-linux-musl/release/creno-relay)" -gt 1000000 \
 && install -D -m 0755 \
      target/x86_64-unknown-linux-musl/release/creno-relay \
      /out/creno-relay

FROM alpine:3.24 AS runtime

# ca-certificates: rustls-platform-verifier reads the system store
# (rustls-native-certs) to validate the upstream TLS certificate.
# uid/gid 10001: unprivileged, no shell, no home, no writable path in the image.
RUN apk add --no-cache ca-certificates \
 && addgroup -g 10001 -S creno \
 && adduser -u 10001 -S -D -H -G creno -s /sbin/nologin creno

COPY --from=builder --chown=10001:10001 /out/creno-relay /usr/local/bin/creno-relay

# Inside a container the relay must accept connections from the bridge network.
# Keep the host side on loopback (see deploy/compose.yml).
ENV CRENO_BIND=0.0.0.0:8080

USER 10001:10001
EXPOSE 8080
STOPSIGNAL SIGTERM

# busybox wget ships with Alpine: no extra package for the probe.
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD wget -q -O- http://127.0.0.1:8080/healthz || exit 1

ENTRYPOINT ["/usr/local/bin/creno-relay"]
