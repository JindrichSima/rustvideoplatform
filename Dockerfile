FROM rust:alpine AS builder

RUN apk add --no-cache musl-dev openssl-dev pkgconfig nodejs npm woff2 openssl-dev

RUN mkdir /src
COPY ./ /src/rustvideoplatform

RUN cd /src/rustvideoplatform && npm install --ignore-scripts && cargo build --release


FROM alpine:latest
RUN apk add --no-cache ffmpeg woff2 openssl
COPY --from=builder /src/rustvideoplatform/target/release/rustvideoplatform /opt/rustvideoplatform

EXPOSE 8080
STOPSIGNAL SIGTERM

ENTRYPOINT ["/opt/rustvideoplatform"]
