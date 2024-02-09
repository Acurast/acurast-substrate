# Acurast Marketplace Pallet
## 🚧🚧🚧 The project is still a work in progress 🚧🚧🚧

## Introduction

The Acurast Marketplace Pallet extends the Acurast Pallet by resource advertisements and matching of registered jobs with suitable sources.

The Pallet exposes a number of extrinsics additional to the strongly coupled (and required) core Marketplace Pallet.

### advertise

Allows the advertisement of resources by a source. An advertisement consists of:

- A list of `pricing` options, each stating resource pricing for a selected reward type.
- The total `capacity` not to be exceeded in matching.
- A list of `allowed_consumers`.

## Benchmarking

Finding weights by means of benchmarking works a bit different for this pallet. The hooks contribute weight to extrinsics
that are defined in parent `pallet_acurast` that is tightly coupled to this pallet and calls into specific behaviour via hooks.

**Therefore this pallet provides `weights_with_hooks::WeightInfoWithHooks` to be used _in place of_ the default weights for `pallet_acurast`.**

### Test benchmarks in this pallet

```shell
cargo test --package pallet-acurast --features runtime-benchmarks
```

```shell
cargo test --package pallet-acurast-marketplace --features runtime-benchmarks
```

### Generate weights
```shell
cargo clean && cargo build --profile=production --features runtime-benchmarks
```

Check if the expected extrinsics are listed
```shell
../../../acurast-substrate/target/production/acurast-node  benchmark pallet --list
```
Should contain:
```text
pallet_acurast, register
pallet_acurast, deregister
pallet_acurast, update_allowed_sources
pallet_acurast, fulfill
pallet_acurast, submit_attestation
pallet_acurast, update_certificate_revocation_list
pallet_acurast_marketplace, advertise
pallet_acurast_marketplace, delete_advertisement
pallet_acurast_marketplace, register
pallet_acurast_marketplace, deregister
pallet_acurast_marketplace, fulfill
```

Run benchmarks:
```shell
../../../acurast-substrate/target/release/acurast-node benchmark pallet --chain=acurast-dev --execution=wasm --wasm-execution=compiled --pallet=pallet_acurast --extrinsic "*" --steps=50 --repeat=20 --output=../acurast/src/weights.rs
```

```shell
../../../acurast-substrate/target/release/acurast-node benchmark pallet --chain=acurast-dev --execution=wasm --wasm-execution=compiled --pallet=pallet_acurast_marketplace --extrinsic "advertise,delete_advertisement" --steps=50 --repeat=20 --output=./src/weights.rs --template=./src/weights.hbs
```
```shell
../../../acurast-substrate/target/release/acurast-node benchmark pallet --chain=acurast-dev --execution=wasm --wasm-execution=compiled --pallet=pallet_acurast_marketplace --extrinsic "register,deregister,fulfill,update_allowed_sources" --steps=50 --repeat=20 --output=./src/weights_with_hooks.rs --template=./src/weights_with_hooks.hbs
```
