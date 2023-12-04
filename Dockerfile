###########
# Builder #
###########
FROM rust:1.72-slim as builder

RUN apt-get update && apt-get install -y pkg-config g++ libssl-dev libpq-dev libpq5

# Copy only list of dependencies and make dummy main.rs to cache deps
WORKDIR /searxiv
COPY . .
RUN cargo build --release --bin arxiv-search

###########
# Runtime #
###########
FROM debian:buster-slim AS runtime

RUN apt-get update && apt-get install -y libpq5

# Copy the binary from build container
COPY --from=builder /searxiv/target/release/arxiv-search /arxiv-search

RUN ldd /arxiv-search

ENTRYPOINT ["/arxiv-search", "server"]

