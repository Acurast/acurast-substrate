#!/usr/bin/env bash

set -eu

cargo +stable contract build --manifest-path validator/Cargo.toml
cargo +stable contract build --manifest-path state/Cargo.toml
cargo +stable contract build --manifest-path proxy/Cargo.toml
cargo +stable contract build --manifest-path consumer/Cargo.toml
