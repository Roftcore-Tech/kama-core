# Kama hub imajı — KM-DEPLOY-TDD-001 v0.5 §4. Elle derlenmez: `.github/workflows/hub-image.yml` üretir.
#
# Aşama 1: repodaki toolchain (rust-toolchain.toml 1.96.0) + musl hedefi → statik `hub-server`.
#          Hub'ın bağımlılıkları saf Rust'tır (C kütüphanesi yok); CI ikilinin "not a dynamic
#          executable" olduğunu ayrıca denetler.
# Aşama 2: boş (`scratch`) imaj — yalnız ikili; root değil (65532); yapılandırma salt-okunur bind mount.
#
# Derleme bağlamı depo KÖKÜDÜR (workspace üyelerinin manifestleri gerekir; .dockerignore target/,
# .git ve belgeleri dışarıda bırakır):
#   docker build -f deploy/Dockerfile -t kama-hub:<kısa-sha> .

FROM rust:1.96.0-bookworm AS build
WORKDIR /src
RUN rustup target add x86_64-unknown-linux-musl
COPY . .
RUN cargo build --locked --release -p hub-server --target x86_64-unknown-linux-musl \
 && cp target/x86_64-unknown-linux-musl/release/hub-server /hub-server

FROM scratch
COPY --from=build /hub-server /hub-server
USER 65532:65532
ENTRYPOINT ["/hub-server", "--config", "/config/hub.json"]
