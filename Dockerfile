FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release --bin meme-host

# We do not need the Rust toolchain to run the binary!
FROM debian:bookworm-slim AS runtime
ARG APP=/user/local/bin

WORKDIR $APP

COPY --from=builder /app/target/release/meme-host $APP/meme-host
COPY --from=builder /app/assets $APP/assets

ENV STATIC_ASSETS=$APP/assets
ENV BASE_SITE_URL=https://meme.crabenjoyer.xyz

ENTRYPOINT ["./meme-host"]
