# Acurast P256 crypto
## 🚧🚧🚧 The project is still a work in progress 🚧🚧🚧

## Introduction

This crate provides types that allow to add P256 (a.k.a secp256r1) signature verification support to substrate based chains.

### Setup

Add the following dependency to your Cargo manifest:

```toml
[dependencies]
acurast-p256-crypto = { git = "https://github.com/Acurast/acurast-core.git" }
```

### Integration

Use the `acurast_p256_crypto::MultiSignature` as your parachain `Signature` type:

```rust
use acurast_p256_crypto::MultiSignature;

pub type Signature = MultiSignature;
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
...

impl frame_system::Config for Runtime {
	type AccountId = AccountId;
    ...
}
```