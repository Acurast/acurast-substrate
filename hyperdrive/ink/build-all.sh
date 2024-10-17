#!/usr/bin/env bash

set -eu

cargo +stable contract build --manifest-path proxy/Cargo.toml
cargo +stable contract build --manifest-path consumer/Cargo.toml
cargo +stable contract build --manifest-path ibc-ink/Cargo.toml
