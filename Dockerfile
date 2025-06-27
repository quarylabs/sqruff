
# Builder stage
FROM rust:1.88-bookworm AS builder

WORKDIR /usr/src/sqruff
COPY . .
RUN cargo build --release -p sqruff --bin sqruff --locked

# Runtime stage
FROM debian:bookworm-slim

COPY --from=builder /usr/src/sqruff/target/release/sqruff /usr/local/bin/sqruff
ENTRYPOINT ["sqruff"]