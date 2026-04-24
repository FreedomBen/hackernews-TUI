FROM rust as builder
WORKDIR app
COPY . .
RUN cargo build --release --bin hackernews_tim

FROM scratch
WORKDIR app
COPY --from=builder /app/target/release/hackernews_tim .
COPY ./examples/config.toml ./config.toml
CMD ["./hackernews_tim", "-l", ".", "-c", "./config.toml"]
