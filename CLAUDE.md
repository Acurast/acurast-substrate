# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

### Standard Build Commands
- `cargo build --release` - Build for Rococo testnet (default)

### Check Commands
- `cargo check --release` - Check compilation for Rococo

### Testing
- `cargo test` - Run all tests
- `cargo test [testname]` - Run specific test by name

## Project Architecture

This is a Substrate-based parachain built on the Cumulus framework, implementing the Acurast protocol for decentralized serverless computing.

### Key Components

**Core Pallets:**
- `pallet-acurast` - Main pallet for job registration and management
- `pallet-acurast-marketplace` - Economic logic for assigning processors to jobs and rewards
- `pallet-acurast-compute` - Compute resource management with hierarchical staking pools and epoch-based rewards
- `pallet-acurast-processor-manager` - Manager and processor registration/management
- `pallet-acurast-hyperdrive` - Cross-chain functionality for Substrate chains
- `pallet-acurast-hyperdrive-ibc` - IBC (Inter-Blockchain Communication) support
- `pallet-acurast-hyperdrive-token` - Cross-chain token bridging

**Runtime Configurations:**
- `acurast-mainnet-runtime` - Production Polkadot parachain runtime
- `acurast-kusama-runtime` - Canary network runtime  
- `acurast-rococo-runtime` - Testnet runtime

### Runtime Structure

The runtime uses Substrate's `construct_runtime!` macro with indexed pallets. Each network variant (mainnet, kusama, rococo) has its own runtime configuration with different features and parameters.

### Testing Strategy

Tests are organized per pallet with:
- Unit tests in `tests.rs` files
- Mock runtime configurations in `mock.rs`
- Stub variables used by both benchmarking and test code in `stub.rs`
- Benchmarking support via `benchmarking.rs`
- Integration tests in `emulations/src/tests`

### Development Notes

- The project uses Polkadot SDK v1.18.5 as the base framework
- Storage migrations are handled per pallet with versioning