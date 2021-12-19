FROM debian:bullseye-slim AS runner
RUN apt update && apt upgrade -yq && apt install -yq docker.io git && rm -vrf /var/lib/apt/sources.list.d/

FROM rust:latest AS builder
WORKDIR /code
# Here, we will cache the dependencies
COPY Cargo.toml .
COPY resources/dummy.rs src/dummy.rs
RUN sed -i 's/main/dummy/' Cargo.toml && sed -i 's%lib/mod.rs%dummy.rs%' Cargo.toml
RUN cargo build --release
RUN rm Cargo.toml
# now, build as normal
COPY . /code
RUN cargo build --release

FROM runner
WORKDIR /app
RUN touch fake-ci.yml
COPY --from=builder /code/target/release/fake-ci .
# always good practice to include the source code used to build the binary
COPY --from=builder /code/src src
COPY --from=builder /code/resources src/resources
USER 1000
VOLUME /tmp
VOLUME ~/.config/fake-ci
CMD ["./fake-ci", "watch"]
