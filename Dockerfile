FROM docker.io/rust:1.64.0 as cargo-build

WORKDIR /tmp/milkman-bot
COPY . /tmp/milkman-bot

RUN apt-get update && apt-get install -y git libssl-dev pkg-config

COPY Cargo.toml /tmp/milkman-bot
COPY Cargo.lock /tmp/milkman-bot

# To cache dependencies, create a layer that compiles dependencies and some rust src that won't change, 
# and then another layer that compiles our source.
RUN echo 'fn main() {}' >> /tmp/milkman-bot/dummy.rs

RUN sed -i 's|src/main.rs|dummy.rs|g' Cargo.toml
RUN env CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --release

RUN sed -i 's|dummy.rs|src/main.rs|g' Cargo.toml
COPY . /tmp/milkman-bot
RUN env CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --release

FROM docker.io/debian:bullseye-slim

COPY --from=cargo-build /tmp/milkman-bot/target/release/milkman-bot /
COPY --from=cargo-build /tmp/milkman-bot /project/
WORKDIR /

RUN apt-get update && apt-get install -y libssl-dev ca-certificates

CMD ["./milkman-bot"]
