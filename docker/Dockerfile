FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /skynav
RUN rustup target add wasm32-unknown-unknown
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
RUN cargo binstall -y trunk

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /skynav/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
RUN cargo chef cook --release --target wasm32-unknown-unknown -p skynav-app --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin skynav-server

WORKDIR /skynav/app
RUN trunk build --release

FROM gcr.io/distroless/cc-debian13:nonroot AS runtime
WORKDIR /skynav
COPY --from=builder /skynav/target/release/skynav-server /usr/local/bin/server
COPY --from=builder /skynav/app/dist /skynav/static
ENV STATIC_DIR=/skynav/static
CMD ["/usr/local/bin/server"]
