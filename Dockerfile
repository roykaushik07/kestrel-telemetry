FROM rust:1.79-slim-bookworm AS build
RUN apt-get update && apt-get install -y --no-install-recommends cmake build-essential pkg-config libssl-dev git && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY Cargo.toml ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 kestrel
WORKDIR /app
COPY --from=build /src/target/release/kestrel-telemetry /usr/local/bin/kestrel-telemetry
COPY config ./config
USER 10001
ENV KESTREL_ENV=production
EXPOSE 9090
ENTRYPOINT ["/usr/local/bin/kestrel-telemetry"]
