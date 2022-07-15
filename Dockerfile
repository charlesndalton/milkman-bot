FROM docker.io/clux/muslrust:1.59.0 as cargo-build

WORKDIR /tmp/cow-order-execution-bot
COPY Cargo.toml /tmp/cow-order-execution-bot
COPY Cargo.lock /tmp/cow-order-execution-bot

# To cache dependencies, create a layer that compiles dependencies and some rust src that won't change, 
# and then another layer that compiles our source.
RUN echo 'fn main() {}' >> /tmp/cow-order-execution-bot/dummy.rs

RUN sed -i 's|src/main.rs|dummy.rs|g' Cargo.toml
RUN env CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --target x86_64-unknown-linux-musl --release

RUN sed -i 's|dummy.rs|src/main.rs|g' Cargo.toml
COPY . /tmp/cow-order-execution-bot
RUN env CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --target x86_64-unknown-linux-musl --release


FROM docker.io/alpine:latest

RUN apk add --no-cache tini

COPY --from=cargo-build /tmp/cow-order-execution-bot/target/x86_64-unknown-linux-musl/release/cow-order-execution-bot /
WORKDIR /

ENV RUST_LOG=INFO
CMD ["./cow-order-execution-bot"]
