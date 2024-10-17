#!/usr/bin/env bash

set -eu

cargo +stable test --manifest-path validator/Cargo.toml
cargo +stable test --manifest-path state/Cargo.toml
cargo +stable test --manifest-path proxy/Cargo.toml
cargo +stable test --manifest-path consumer/Cargo.toml
