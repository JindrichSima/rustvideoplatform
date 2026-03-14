FROM rust:alpine AS builder

RUN apk add --no-cache musl-dev pkgconfig nodejs npm woff2 openssl-dev perl

RUN mkdir /src
COPY ./ /src/rustvideoplatform

ARG TARGETARCH
RUN case "$TARGETARCH" in \
        amd64)   export RUSTFLAGS="-C target-cpu=x86-64-v3" ;; \
        ppc64le) export RUSTFLAGS="-C target-cpu=pwr8" ;; \
    esac && \
    cd /src/rustvideoplatform && npm install --ignore-scripts && cargo build --release


FROM alpine:latest
RUN apk add --no-cache ffmpeg woff2
COPY --from=builder /src/rustvideoplatform/target/release/rustvideoplatform /opt/rustvideoplatform

EXPOSE 8080
STOPSIGNAL SIGTERM

ENTRYPOINT ["/opt/rustvideoplatform"]
