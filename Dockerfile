# syntax=docker/dockerfile:1
FROM alpine:edge AS builder

RUN apk add --no-cache cargo musl-dev pkgconfig nodejs npm woff2 openssl-dev perl make build-base

ARG TARGETARCH

# Pre-build dependencies (cached layer - only invalidated when Cargo.toml changes)
COPY Cargo.toml /src/rustvideoplatform/
RUN mkdir -p /src/rustvideoplatform/src && echo 'fn main() {}' > /src/rustvideoplatform/src/main.rs
RUN --mount=type=cache,target=/root/.cargo/registry,id=cargo-reg-${TARGETARCH},sharing=locked \
    --mount=type=cache,target=/root/.cargo/git,id=cargo-git-${TARGETARCH},sharing=locked \
    case "$TARGETARCH" in \
        amd64)   export RUSTFLAGS="-C target-cpu=x86-64-v3" ;; \
        ppc64le) export RUSTFLAGS="-C target-cpu=pwr8" ;; \
    esac && \
    cd /src/rustvideoplatform && cargo build --release 2>/dev/null ; true

# Build actual project
COPY ./ /src/rustvideoplatform
RUN --mount=type=cache,target=/root/.cargo/registry,id=cargo-reg-${TARGETARCH},sharing=locked \
    --mount=type=cache,target=/root/.cargo/git,id=cargo-git-${TARGETARCH},sharing=locked \
    case "$TARGETARCH" in \
        amd64)   export RUSTFLAGS="-C target-cpu=x86-64-v3" ;; \
        ppc64le) export RUSTFLAGS="-C target-cpu=pwr8" ;; \
    esac && \
    cd /src/rustvideoplatform && npm install --ignore-scripts && cargo build --release


FROM alpine:edge
RUN apk add --no-cache ffmpeg woff2
COPY --from=builder /src/rustvideoplatform/target/release/rustvideoplatform /opt/rustvideoplatform

EXPOSE 8080
STOPSIGNAL SIGTERM

ENTRYPOINT ["/opt/rustvideoplatform"]
