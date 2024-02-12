# Acurast Pallet
## 🚧🚧🚧 The project is still a work in progress 🚧🚧🚧

## Introduction

The Acurast Pallet allows a Parachain to integrate the Acurast functionality to be able to securly receive real world data posted by the Acurast Processors.

The Pallet exposes a number of extrinsic.

### register

Allows the registration of a job. A registration consists of:

- An ipfs URL to a `script` (written in Javascript).
    - The script will be run in the Acurast Trusted Virtual Machine that uses a Trusted Execution Environment (TEE) on the Acurast Processor.
- An optional `allowedSources` list of allowed sources.
    - A list of `AccountId`s that are allowed to `fulfill` the job. If no list is provided, all sources are accepted.
- An `allowOnlyVerifiedSources` boolean indicating if only verified source can fulfill the job.
    - A verified source is one that has provided a valid key attestation.
- An `extra` structure that can be used to provide custom parameters.

Registrations are saved per `AccountId` and `script`, meaning that `register` is called twice from the same `AccountId` with the same `script` value, the previous registration is overwritten.

### deregister

Allows the de-registration of a job.

### updateAllowedSources

Allows to update the list of allowed sources for a previously registered job.

### submitAttestation

Allows an Acurast Processor to submit a key attestation proving its integrity. The extrinsic parameter is a valid attestation certificate chain.

### updateCertificateRevocationList

Allows to update the certificate recovation list used during attestation validation.

## Setup

Add the following dependency to your Cargo manifest:

```toml
[dependencies]
pallet-acurast = { git = "https://github.com/Acurast/acurast-core.git" }
```

## Parachain Integration

Implement `pallet_acurast::Config` for your `Runtime` and add the Pallet:

```rust
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
pub struct AcurastRegistrationExtra {
	/// my extra registration parameters
}

pub type MaxAllowedSources = CU32<10>;

parameter_types! {
	pub const AcurastPalletId: PalletId = PalletId(*b"acrstpid");
}

impl pallet_acurast::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RegistrationExtra = AcurastRegistrationExtra;
	type MaxAllowedSources = MaxAllowedSources;
	type RewardManager = (); // provide proper type to enable rewards to be payed on fulfillment
	type PalletId = AcurastPalletId;
	type RevocationListUpdateBarrier = ();
	type KeyAttestationBarrier = ();
	type UnixTime = pallet_timestamp::Pallet<Self>;
	type WeightInfo = pallet_acurast::weights::WeightInfo<Self>;
}

// Create the runtime by composing the FRAME pallets that were previously configured.
construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = opaque::Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		// All your other pallets
        ...
		// Acurast
		Acurast: pallet_acurast::{Pallet, Call, Storage, Event<T>} = 50,
	}
);
```

## P256 signatures

Acurast Processors will sign extrinsics (the `fulfill` call) using a P256 (a.k.a secp256r1) private key.

By default, Substrate does not support the P256 curve. Use the `acurast-p256-crypto` crate to add support for P256 signature verification.

To do so, use the `acurast_p256_crypto::MultiSignature` as your parachain `Signature` type:

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
