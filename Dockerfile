FROM rust:1.62 AS builder
RUN apt update && apt install --assume-yes git clang curl libssl-dev llvm libudev-dev make protobuf-compiler
RUN rustup update nightly && rustup target add wasm32-unknown-unknown --toolchain nightly

WORKDIR /code
COPY . .

RUN cargo build --release

# adapted from https://github.com/paritytech/polkadot/blob/master/scripts/ci/dockerfiles/polkadot/polkadot_builder.Dockerfile
FROM docker.io/library/ubuntu:20.04

COPY --from=builder /code/target/release/parachain-collator /usr/local/bin/

RUN useradd -m -u 1000 -U -s /bin/sh -d /app app && \
	mkdir /data && \
	chown -R app:app /data && \
# check if executable works in this container
	/usr/local/bin/parachain-collator --version

USER app

ENTRYPOINT ["/usr/local/bin/parachain-collator"]
CMD [ "help" ]