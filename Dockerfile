# syntax=docker/dockerfile:1

# ---------- Сборка: статический бинарник под musl ----------
FROM rust:1-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src

# Сначала только манифесты и пустой main.rs: слой с зависимостями кешируется
# и пересобирается, только когда меняется Cargo.toml / Cargo.lock.
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs \
    && cargo build --release \
    && rm -rf src

# Теперь сам код.
COPY src ./src
RUN touch src/main.rs \
    && cargo build --release \
    && mkdir -p /out/data \
    && cp target/release/pokebonk-lolka /out/

# ---------- Итоговый образ: только бинарник (~10 МБ) ----------
# Корневые сертификаты встроены в бинарник (webpki-roots), поэтому
# образ может быть пустым (scratch).
FROM scratch
COPY --from=build /out/pokebonk-lolka /pokebonk-lolka
COPY --from=build --chown=65534:65534 /out/data /data
USER 65534:65534
ENV STATE_PATH=/data/state.json
VOLUME ["/data"]
ENTRYPOINT ["/pokebonk-lolka"]
