FROM rust:1.75 AS builder
RUN apt update && apt install --assume-yes git clang curl libssl-dev llvm libudev-dev make protobuf-compiler build-essential
RUN rustup update nightly-2023-10-31
RUN rustup target add wasm32-unknown-unknown --toolchain nightly-2023-10-31
RUN rustup component add rust-src --toolchain nightly-2023-10-31

WORKDIR /code
COPY . .

ARG chain=""
ARG benchmarks=""

RUN \
if [ "${benchmarks}" = "kusama" ] ; then \
    cargo build --no-default-features --features 'acurast-kusama,std,runtime-benchmarks' --release ; \
elif [[ -n "${benchmarks}" ]] ; then \
    cargo build --no-default-features --features 'runtime-benchmarks' --release ; \
elif [ "${chain}" = "kusama" ] ; then \
	cargo build --no-default-features --features 'acurast-kusama,std,allow-faucet' --release ; \
else \
    cargo build --release ; \
fi

# adapted from https://github.com/paritytech/polkadot/blob/master/scripts/ci/dockerfiles/polkadot/polkadot_builder.Dockerfile
FROM docker.io/library/ubuntu:22.04

COPY --from=builder /code/target/release/acurast-node /usr/local/bin/
COPY chain-specs /chain-specs

RUN useradd -m -u 1000 -U -s /bin/sh -d /app app && \
	mkdir -p /data /app/.local/share && \
	chown -R app:app /data && \
	ln -s /data /app/.local/share/app && \
# unclutter and minimize the attack surface
	rm -rf /usr/bin /usr/sbin && \
# check if executable works in this container
	/usr/local/bin/acurast-node --version

USER app

ENTRYPOINT ["/usr/local/bin/acurast-node"]
CMD [ "help" ]
