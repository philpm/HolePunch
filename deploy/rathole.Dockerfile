FROM rust:1.91.1-bookworm AS build
RUN apt-get update && apt-get install -y --no-install-recommends python3 && rm -rf /var/lib/apt/lists/*
WORKDIR /src
RUN curl -fsSL https://static.crates.io/crates/rathole/rathole-0.5.0.crate -o rathole.crate && echo '6358cf83f700408be4176f5bb96976404366154c384185cd79cfa8f43087ae4f  rathole.crate' | sha256sum -c - && tar -xzf rathole.crate
COPY scripts/patch-rathole.py /src/patch-rathole.py
RUN python3 /src/patch-rathole.py /src/rathole-0.5.0
COPY deploy/rathole.Cargo.lock /src/rathole-0.5.0/Cargo.lock
RUN cargo build --manifest-path rathole-0.5.0/Cargo.toml --release --locked --no-default-features --features server,client,noise,hot-reload
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/* && useradd -u 10001 -m holepunch
COPY --from=build /src/rathole-0.5.0/target/release/rathole /usr/local/bin/
USER 10001:10001
ENTRYPOINT ["rathole","--server","/data/server.toml"]
