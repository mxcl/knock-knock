FROM node:26-bookworm-slim AS web
WORKDIR /build/web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

FROM rust:1.96-bookworm AS backend
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY migrations/ migrations/
COPY src/ src/
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=backend /build/target/release/knock-knock /usr/local/bin/knock-knock
COPY --from=web /build/web/dist/ web/dist/
ENV BIND_ADDR=0.0.0.0:3000
EXPOSE 3000
CMD ["knock-knock"]

