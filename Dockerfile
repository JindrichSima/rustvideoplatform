FROM alpine:latest AS builder

RUN apk add --no-cache cargo musl-dev openssl-dev pkgconfig ffmpeg-dev clang19-dev

RUN mkdir /src
COPY ./ /src/rustvideoplatform

ENV RUSTFLAGS="-C target-cpu=x86-64-v2"
RUN cd /src/rustvideoplatform && cargo build --release


FROM alpine:latest
COPY --from=builder /src/rustvideoplatform/target/release/rustvideoplatform /opt/rustvideoplatform

RUN apk add --no-cache ffmpeg libva libva-utils mesa-dri-gallium mesa-va-gallium intel-media-driver

EXPOSE 8080
STOPSIGNAL SIGTERM

ENTRYPOINT ["/opt/rustvideoplatform"]