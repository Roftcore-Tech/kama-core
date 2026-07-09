# harness/Dockerfile — kama-core bench (quinn baseline). rust:1.96 (rust-toolchain.toml ile hizalı;
# runbook D.1'in rust:1.83'ünden sapma — gerekçeli). hub-server YOK; iki uç bench app koşar.
FROM rust:1.96-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    iproute2 iperf3 python3 ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
RUN cargo build --release -p bench
