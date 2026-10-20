FROM rust:1.91.1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY web ./web
COPY apps/selfhost ./apps/selfhost
COPY apps/cli ./apps/cli
RUN cargo build --release --locked -p holepunch-server && cp target/release/holepunch-server /server
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/* && useradd -u 10001 -m holepunch && mkdir /data && chown 10001:10001 /data
COPY --from=build /server /usr/local/bin/holepunch-center
USER 10001:10001
ENV HP_DATA_DIR=/data HP_BIND=127.0.0.1:3000
ENTRYPOINT ["holepunch-center"]
