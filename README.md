# Acurast Substrate

Acurast Substrate is a [Cumulus](https://github.com/paritytech/cumulus/)-based parachain. The [Cumulus Parachain Tempalte](https://github.com/paritytech/cumulus/tree/master/parachain-template) was used as the base for the node and runtime implementation.

To learn more about Acurast please visit the [website](https://acurast.com/) and [documentation](https://docs.acurast.com/).

## Build

```
cargo build --release
```

## Run

First build the plain chain spec:

```
./target/release/acurast-node build-spec --disable-default-bootnode > rococo-local-parachain-plain.json
```

In `rococo-local-parachain-plain.json` set the parachain id to 2000 by:

- changing the value of `para_id` at the root level
- changing the value at `genesis.runtime.parachainInfo.parachainId`

Then create the raw version of the chain spec:

```
./target/release/acurast-node build-spec --chain rococo-local-parachain-plain.json --raw --disable-default-bootnode > rococo-local-parachain-2000-raw.json
```

Now run the node with the following command:

```
RUST_LOG=runtime=trace ./target/release/acurast-node --alice --collator --force-authoring --chain rococo-local-parachain-2000-raw.json --base-path /tmp/parachain/alice --rpc-port 8080 --port 40333 --ws-port 8844 --unsafe-rpc-external --unsafe-ws-external --rpc-cors all -- --execution wasm --chain ../polkadot/rococo-local-raw.json --port 30343 --ws-port 9977
```

The above command assumes that there is a rococo relay chain with the raw spec at `../polkadot/rococo-local-raw.json`.

See substrate tutorials on how to setup the replay chain and connect a parachain to it:

- [Start a local relay chain](https://docs.substrate.io/tutorials/connect-other-chains/local-relay/)
- [Connect a local parachain](https://docs.substrate.io/tutorials/connect-other-chains/local-parachain/)
