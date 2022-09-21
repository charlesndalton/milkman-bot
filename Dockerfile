FROM docker.io/clux/muslrust:1.59.0 as cargo-build

WORKDIR /tmp/milkman-bot
COPY Cargo.toml /tmp/milkman-bot
COPY Cargo.lock /tmp/milkman-bot

# To cache dependencies, create a layer that compiles dependencies and some rust src that won't change, 
# and then another layer that compiles our source.
RUN echo 'fn main() {}' >> /tmp/milkman-bot/dummy.rs

RUN sed -i 's|src/main.rs|dummy.rs|g' Cargo.toml
RUN env CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --target x86_64-unknown-linux-musl --release

RUN sed -i 's|dummy.rs|src/main.rs|g' Cargo.toml
COPY . /tmp/milkman-bot
RUN env CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --target x86_64-unknown-linux-musl --release


FROM docker.io/debian:bullseye-slim

COPY --from=cargo-build /tmp/milkman-bot/target/x86_64-unknown-linux-musl/release/milkman-bot /
WORKDIR /

RUN apt-get update && apt-get install -y ca-certificates tini && apt-get clean

ENV RUST_LOG=INFO
ENTRYPOINT ["/usr/bin/tini", "--"]

CMD ["./milkman-bot"]
