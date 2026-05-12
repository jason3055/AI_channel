FROM rust:1-slim-bookworm AS builder

WORKDIR /app
COPY . .
RUN cargo build --release -p aichan-server

FROM debian:bookworm-slim

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/*
RUN useradd --system --uid 10001 --home /nonexistent --shell /usr/sbin/nologin aichan
COPY --from=builder /app/target/release/aichan-server /usr/local/bin/aichan-server

ENV PORT=8080
ENV AICHAN_DATA_DIR=/tmp/aichan-server
EXPOSE 8080

USER aichan
CMD ["aichan-server"]
