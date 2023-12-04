###########
# Builder #
###########
FROM rust:1.72-bookworm as builder

RUN apt-get update && \
    apt-get install -y pkg-config g++ libssl-dev libpq-dev

RUN wget https://download.pytorch.org/libtorch/cpu/libtorch-cxx11-abi-shared-with-deps-2.1.0%2Bcpu.zip && \
    unzip libtorch-cxx11-abi-shared-with-deps-2.1.0+cpu.zip && \
    rm libtorch-cxx11-abi-shared-with-deps-2.1.0+cpu.zip

ENV LIBTORCH=/libtorch
ENV LD_LIBRARY_PATH=$LD_LIBRARY_PATH:$LIBTORCH/lib

WORKDIR /searxiv
COPY . .
RUN cargo build --release --bin arxiv-search

###########
# Runtime #
###########
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y libpq5 openssl libgomp1

COPY --from=builder /libtorch /libtorch
ENV LIBTORCH=/libtorch
ENV LD_LIBRARY_PATH=$LD_LIBRARY_PATH:$LIBTORCH/lib

COPY --from=builder /searxiv/target/release/arxiv-search /arxiv-search

ENTRYPOINT ["/arxiv-search", "server"]

