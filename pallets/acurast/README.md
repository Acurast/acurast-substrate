# Acurast Pallet
## 🚧🚧🚧 The project is still a work in progress 🚧🚧🚧

## Introduction

The Acurast Pallet allows a Parachain to integrate the Acurast functionality to be able to securly receive real world data posted by the Acurast Data Transmitters.

The Pallet exposes a number of extrinsic.

### register

Allows the registration of a job. A registration consists of:

- An ipfs URL to a `script` (written in Javascript).
    - The script will be run in the Acurast Trusted Virtual Machine that uses a Trusted Execution Environment (TEE) on the Acurast Data Transmitter.
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

### fulfill

Allows to post the fulfillment of a registered job. The fulfillment structure consists of:

- The ipfs url of the `script` executed.
- The `payload` bytes representing the output of the `script`.

In addition to the `fulfillment` structure, `fulfill` expects the `AccountId` of the `requester` of the job.

## Setup

Add the following dependency to your Cargo manifest:

```toml
[dependencies]
pallet-acurast = { git = "https://github.com/Acurast/acurast.git", tag = "0.0.1" }
```

## Parachain Integration

Implement `pallet_acurast::Config` for your `Runtime` and add the Pallet:

```rust
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
pub struct AcurastRegistrationExtra {
	/// my extra registration parameters
}

/// My fulfillment router
pub struct AcurastRouter;
impl pallet_acurast::FulfillmentRouter<Runtime> for AcurastRouter {
	fn received_fulfillment(
		origin: frame_system::pallet_prelude::OriginFor<Runtime>,
		from: <Runtime as frame_system::Config>::AccountId,
		fulfillment: pallet_acurast::Fulfillment,
		registration: pallet_acurast::Registration<AcurastRegistrationExtra>,
		requester: <<Runtime as frame_system::Config>::Lookup as StaticLookup>::Target,
	) -> DispatchResultWithPostInfo {
		/// route the fulfillment to its final destination
	}
}

impl pallet_acurast::Config for Runtime {
	type Event = Event;
	type RegistrationExtra = AcurastRegistrationExtra;
	type FulfillmentRouter = AcurastRouter;
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

### Example integration with EVM parachain

The following example shows a possible integration approach for an EVM parachain (using the [frontier](https://github.com/paritytech/frontier)).
The example shows how to route the fulfillment's pyload to a smart contract by calling the `fulfill` mehod on it and passing the payload bytes are argument.

```rust
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
pub enum MethodSignatureHash {
	Default,
	Custom(BoundedVec<u8, ConstU32<4>>),
}

impl MethodSignatureHash {
	fn to_bytes(&self) -> [u8; 4] {
		match self {
			Self::Default => keccak_256!(b"fulfill(address,bytes)")[0..4].try_into().unwrap(),
			Self::Custom(bytes) => bytes.to_vec().try_into().unwrap(),
		}
	}
}

#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
pub struct AcurastRegistrationExtra {
	pub destination_contract: H160,
	pub method_signature_hash: MethodSignatureHash,
}

pub struct AcurastRouter;
impl pallet_acurast::FulfillmentRouter<Runtime> for AcurastRouter {
	fn received_fulfillment(
		origin: frame_system::pallet_prelude::OriginFor<Runtime>,
		from: <Runtime as frame_system::Config>::AccountId,
		fulfillment: pallet_acurast::Fulfillment,
		registration: pallet_acurast::Registration<AcurastRegistrationExtra>,
		requester: <<Runtime as frame_system::Config>::Lookup as StaticLookup>::Target,
	) -> DispatchResultWithPostInfo {
		let from_bytes: [u8; 32] = from.try_into().unwrap();
		let eth_source = H160::from_slice(&from_bytes[0..20]);
		let requester_bytes: [u8; 32] = requester.try_into().unwrap();
		let eth_requester = H160::from_slice(&requester_bytes[0..20]);
		let gas_limit = 4294967;
		EVM::call(
			origin,
			eth_source,
			registration.extra.destination_contract,
			create_eth_call(
				registration.extra.method_signature_hash,
				eth_requester,
				fulfillment.payload,
			),
			U256::zero(),
			gas_limit,
			DefaultBaseFeePerGas::get(),
			None,
			None,
			vec![],
		)
	}
}

fn create_eth_call(method: MethodSignatureHash, requester: H160, payload: Vec<u8>) -> Vec<u8> {
	let mut requester_bytes: [u8; 32] = [0; 32];
	requester_bytes[(32 - requester.0.len())..].copy_from_slice(&requester.0);
	let mut offset_bytes: [u8; 32] = [0; 32];
	let payload_offset = requester_bytes.len().to_be_bytes();
	offset_bytes[(32 - payload_offset.len())..].copy_from_slice(&payload_offset);
	let mut payload_len_bytes: [u8; 32] = [0; 32];
	let payload_len = payload.len().to_be_bytes();
	payload_len_bytes[(32 - payload_len.len())..].copy_from_slice(&payload_len);
	[
		method.to_bytes().as_slice(),
		requester_bytes.as_slice(),
		offset_bytes.as_slice(),
		payload_len_bytes.as_slice(),
		&payload,
	]
	.concat()
}

impl pallet_acurast::Config for Runtime {
	type Event = Event;
	type RegistrationExtra = AcurastRegistrationExtra;
	type FulfillmentRouter = AcurastRouter;
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
		// EVM
		Ethereum: pallet_ethereum::{Pallet, Call, Storage, Event, Config, Origin} = 50,
		EVM: pallet_evm::{Pallet, Config, Call, Storage, Event<T>} = 51,
		BaseFee: pallet_base_fee::{Pallet, Call, Storage, Config<T>, Event} = 52,

		// Acurast
		Acurast: pallet_acurast::{Pallet, Call, Storage, Event<T>} = 60,
	}
);
```

## P256 signatures

Acurast Data Transmitters will sign extrinsics (the `fulfill` call) using a P256 (a.k.a secp256r1) private key.

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


