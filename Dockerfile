
# Builder stage
FROM rust:1.72 AS builder
WORKDIR /usr/src/sqruff
COPY . .
RUN cargo build --release --bin sqruff

# Runtime stage
FROM debian:buster-slim
COPY --from=builder /usr/src/sqruff/target/release/sqruff /usr/local/bin/sqruff
ENTRYPOINT ["sqruff"]