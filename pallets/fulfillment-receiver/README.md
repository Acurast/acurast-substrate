# Acurast Fulfillment Receiver Pallet
## 🚧🚧🚧 The project is still a work in progress 🚧🚧🚧

## Introduction

The Acurast Fullfilment Receiver Pallet, in combination with the [Acurast P256 crypto](../../p256-crypto/README.md) package, allows a Parachain to accepts direct fulfillments from Acurast Processors.

The Pallet exposes one extrinsic.

### fulfill

Allows to post the [Fulfillment] of a job. The fulfillment structure consists of:

- The ipfs url of the `script` executed.
- The `payload` bytes representing the output of the `script`.

## Parachain Integration

Implement `pallet_acurast_fulfillment_receiver::Config` for your `Runtime` and add the Pallet:

```rust
frame_support::construct_runtime!(
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>} = 0,
        AcurastFulfillmentReceiver: pallet_acurast_fulfillment_receiver::{Pallet, Call, Event<T>}
    }
);

impl pallet_acurast_fulfillment_receiver::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnFulfillment = FulfillmentHandler;
    type WeightInfo = ();
}

pub struct FulfillmentHandler;
impl OnFulfillment<Runtime> for FulfillmentHandler {
    fn on_fulfillment(
        from: <Runtime as frame_system::Config>::AccountId,
        _fulfillment: pallet_acurast_fulfillment_receiver::Fulfillment,
    ) -> sp_runtime::DispatchResultWithInfo<frame_support::weights::PostDispatchInfo> {
        /// check if origin is a valid Acurast Processor AccountId
        if !is_valid(&from) {
            return Err(DispatchError::BadOrigin.into());
        }
        /// if valid, then fulfillment can be used
        Ok(().into())
    }
}
```

Provide and implementation of [OnFulfillment] to handle the received fulfillment. The implementation should check that the fulfillment is from a known Acurast Processor account id.

### Example integration with EVM parachain

The following example shows a possible integration approach for an EVM parachain using [frontier](https://github.com/paritytech/frontier).
The example shows how to route the fulfillment's pyload to a smart contract by calling the `fulfill` method on it and passing the payload bytes are argument.

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

pub struct FulfillmentHandler;
impl OnFulfillment<Runtime> for FulfillmentHandler {
	fn on_fulfillment(
        from: <Runtime as frame_system::Config>::AccountId,
        fulfillment: pallet_acurast_fulfillment_receiver::Fulfillment,
    ) -> sp_runtime::DispatchResultWithInfo<frame_support::weights::PostDispatchInfo> {
		let from_bytes: [u8; 32] = from.try_into().unwrap();
		let eth_source = H160::from_slice(&from_bytes[0..20]);
		let gas_limit = 4294967;
        let eth_contract_address = H160::from(....);
		EVM::call(
			origin,
			eth_source.clone(),
			eth_contract_address,
			create_eth_call(
				MethodSignatureHash::Default,
				eth_source,
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
		AcurastFulfillmentReceiver: pallet_acurast_fulfillment_receiver::{Pallet, Call, Event<T>}
	}
);

impl pallet_acurast_fulfillment_receiver::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnFulfillment = FulfillmentHandler;
    type WeightInfo = ();
}
```

The following snippet of code shows a very basic EVM smart contract capable of receiving the routed `fulfill` call from the `FulfillmentHandler` implemented above:

```solidity
pragma solidity ^0.8.0;
contract SimpleFulfill {
    address _address;
    bytes _payload;
    function fulfill(address addr, bytes memory payload) public {
        _address = addr;
        _payload = payload;
    }
    function getAddress() public view returns(address) {
        return _address;
    }
    function getPayload() public view returns(bytes memory) {
        return _payload;
    }
}
```

### Example integration with WASM smart contract parachain

The following example shows a possible integration approach for a WASM smart contract parachain (using [pallet-contracts](https://github.com/paritytech/polkadot-sdk/tree/master/frame/contracts)).
Similarly to the EVM integration, the example shows how to route the fulfillment's payload to a smart contract by calling the `fulfill` method on it and passing the payload bytes as argument.

```rust
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq)]
pub enum ContractMethodSelector {
	Default,
	Custom([u8; 4]),
}

impl ContractMethodSelector {
	fn into_fixed_bytes(self) -> [u8; 4] {
		match self {
			Self::Default => BlakeTwo256::hash(b"fulfill").as_bytes()[0..4].try_into().unwrap(),
			Self::Custom(bytes) => bytes,
		}
	}
}

pub struct FulfillmentHandler;
impl OnFulfillment<Runtime> for FulfillmentHandler {
	fn on_fulfillment(
        from: <Runtime as frame_system::Config>::AccountId,
        fulfillment: pallet_acurast_fulfillment_receiver::Fulfillment,
    ) -> sp_runtime::DispatchResultWithInfo<frame_support::weights::PostDispatchInfo> {
        let contract_address: AccountId = ...
		Contracts::call(
			origin,
			contract_address,
			0,
			18_750_000_000,
			None,
			[
				BlakeTwo256::hash(b"fulfill").as_bytes()[0..4].try_into().unwrap().to_vec(),
				from.encode(),
				from.encode(),
				fulfillment.payload.encode(),
			]
			.concat(),
		)
	}
}

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = opaque::Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		// All your other pallets
		...
		// Contracts
		Contracts: pallet_contracts,

		// Acurast
		Acurast: pallet_acurast_fulfillment_receiver,

	}
);
```

The following snippet of code shows a very basic WASM smart contract implemented using [ink!](https://github.com/paritytech/ink) and capable of receiving the routed `fulfill` call from the `FulfillmentHandler` implemented above:

```rust
#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

#[ink::contract]
mod receiver {

    use ink_prelude::vec::Vec;

    /// Defines the storage of your contract.
    /// Add new fields to the below struct in order
    /// to add new static storage fields to your contract.
    #[ink(storage)]
    pub struct Receiver {
        source: Option<AccountId>,
        target: Option<AccountId>,
        payload: Option<Vec<u8>>,
    }

    impl Receiver {
        /// Constructor that initializes `source`, `target` and `payload` to `None`.
        ///
        /// Constructors can delegate to other constructors.
        #[ink(constructor)]
        pub fn default() -> Self {
            Self {
                source: None,
                target: None,
                payload: None,
            }
        }

        /// Simply stores the `source`, `target` and `payload` values.
        #[ink(message)]
        pub fn fulfill(&mut self, source: AccountId, target: AccountId, payload: Vec<u8>) {
            self.source = Some(source);
            self.target = Some(target);
            self.payload = Some(payload);
        }

        /// Simply returns the current value of our `source`, `target` and `payload`.
        #[ink(message)]
        pub fn get(&self) -> (Option<AccountId>, Option<AccountId>, Option<Vec<u8>>) {
            (
                self.source.clone(),
                self.target.clone(),
                self.payload.clone(),
            )
        }
    }
}
```

## WASM Smart Contract direct integration

It is also possible to directly fulfill to a WASM smart contract without goign through a pallet first. For example, given the following smart contract:

```rust
#![cfg_attr(not(feature = "std"), no_std)]

use ink;

#[ink::contract]
mod receiver {

    use ink::{
        env::{caller, DefaultEnvironment},
        storage::Mapping,
    };

    #[ink(storage)]
    pub struct Receiver {
        allowed_processors: Mapping<AccountId, ()>,
        price: u128,
    }

    impl Receiver {

        #[ink(constructor)]
        pub fn default() -> Self {
            Self {
                allowed_processors: Default::default(),
                price: Default::default(),
            }
        }

        #[ink(message)]
        pub fn fulfill(&mut self, price: u128) {
            let caller = caller::<DefaultEnvironment>();
            if self.allowed_processors.contains(&caller) {
                self.price = price;
            }
        }

        #[ink(message)]
        pub fn get_price(&self) -> u128 {
            self.price
        }

        #[ink(message)]
        pub fn add_processor(&mut self, processor: AccountId) {
            self.allowed_processors.insert(processor, &());
        }

        #[ink(message)]
        pub fn remove_processor(&mut self, processor: AccountId) {
            self.allowed_processors.remove(&processor);
        }
    }
}
```

It is possible to directly call the fulfill method from the processors. The example script below shows how to call the above smart contract deployed on the Shibuya parachain:

```javascript
const callIndex = '0x4606'; // the call index for the "call" extrinsic of pallet-contracts
const payload = _STD_.chains.substrate.codec.encodeUnsignedNumber(2, 128); // encoding the price value (2) as an u128
const destination = "XTj3CLB3G6WnwPMgYo1PM2xi9BkDegtGw5WPd7X36G515La"; // the smart contract address
_STD_.chains.substrate.signer.setSigner("SECP256K1"); // select which curve to use for the signature
_STD_.chains.substrate.contract.fulfill(
    "https://shibuya-rpc.dwellir.com", // the parachain rpc endpoint
    callIndex,
    destination,
    payload,
    {
        refTime: "3951114240",
        proofSize: "629760",
    },
    (opHash) => {
        print("Succeeded: " + opHash)
    },
    (err) => {
        print("Failed: " + err)
    },
)
```
