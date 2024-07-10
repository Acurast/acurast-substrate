#![allow(unused_imports)]

use derive_more::{Display, From, Into};
use frame_support::{
	pallet_prelude::*,
	parameter_types,
	traits::{ConstU16, ConstU64},
	Deserialize, PalletId, Serialize,
};
use frame_system as system;
use frame_system::pallet_prelude::BlockNumberFor;
use hex_literal::hex;
use pallet_acurast::CU32;
use pallet_balances::Instance1;
use sp_core::{H256, *};
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup, Keccak256},
	AccountId32, BuildStorage, MultiSignature,
};
use sp_std::prelude::*;

use crate::{
	stub::{Balance, EXISTENTIAL_DEPOSIT},
	weights, MessageBody, MessageProcessor,
};

type Block = frame_system::mocking::MockBlock<Test>;

parameter_types! {
	pub const AcurastPalletId: PalletId = PalletId(*b"acrstpid");
	pub const MinimumPeriod: u64 = 2000;
}

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Hyperdrive: crate::<Instance1>::{Pallet, Call, Storage, Event<T>, HoldReason},
	}
);

impl system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Nonce = u64;
	type Block = Block;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId32;
	type Lookup = IdentityLookup<Self::AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ConstU16<42>;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type RuntimeTask = ();
}

impl pallet_timestamp::Config for Test {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

parameter_types! {
	pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
}

impl pallet_balances::Config for Test {
	/// The type for recording an account's balance.
	type Balance = Balance;
	type DustRemoval = ();
	/// The ubiquitous event type.
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = pallet_balances::weights::SubstrateWeight<Test>;
	type MaxLocks = ConstU32<0>;
	type MaxReserves = ConstU32<0>;
	type ReserveIdentifier = ();
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = ();
	type FreezeIdentifier = ();
	// Holds are used for fees only.
	type MaxHolds = ConstU32<1>;
	type MaxFreezes = ConstU32<0>;
}

parameter_types! {
	pub MinTTL: BlockNumberFor<Test> = 20;
	pub MinDeliveryConfirmationSignatures: u32 = 1;
	pub MinReceiptConfirmationSignatures: u32 = 1;
}

impl crate::Config<Instance1> for Test {
	type RuntimeEvent = RuntimeEvent;
	type MinTTL = MinTTL;
	type MinDeliveryConfirmationSignatures = MinDeliveryConfirmationSignatures;
	type MinReceiptConfirmationSignatures = MinReceiptConfirmationSignatures;
	type Currency = Balances;
	type RuntimeHoldReason = RuntimeHoldReason;
	type MessageIdHashing = BlakeTwo256;
	type MessageProcessor = ();
	type WeightInfo = weights::WeightInfo<Test>;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	let storage = system::GenesisConfig::<Test>::default().build_storage().unwrap().into();

	let mut ext = sp_io::TestExternalities::new(storage);
	ext.execute_with(|| System::set_block_number(1));
	ext
}

pub fn events() -> Vec<RuntimeEvent> {
	log::debug!("{:#?}", System::events());
	let evt = System::events().into_iter().map(|evt| evt.event).collect::<Vec<_>>();

	System::reset_events();

	evt
}

impl MessageProcessor<AccountId32, AccountId32> for () {
	fn process(_: MessageBody<AccountId32, AccountId32>) -> DispatchResultWithPostInfo {
		Ok(().into())
	}
}
