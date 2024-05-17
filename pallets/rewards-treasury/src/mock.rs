use frame_support::{
	derive_impl, parameter_types,
	sp_runtime::{
		traits::{ConstU16, ConstU32, ConstU64, IdentityLookup},
		BuildStorage,
	},
};
use sp_core::H256;
use sp_std::prelude::*;

use crate::{stub::*, *};

pub struct ExtBuilder;

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {}
	}
}

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>} = 0,
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		RewardsTreasury: crate::{Pallet, Storage, Event<T>}
	}
);

#[derive_impl(frame_system::config_preludes::ParaChainDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Nonce = u64;
	type Hash = H256;
	type Block = Block<Test>;
	type BlockHashCount = ConstU64<250>;
	type Version = ();
	type AccountData = pallet_balances::AccountData<Balance>;
	type DbWeight = ();
	type BlockWeights = ();
	type BlockLength = ();
	type SS58Prefix = ConstU16<42>;
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

parameter_types! {
	pub const ExistentialDeposit: Balance = MICROUNIT;
	pub const MaxReserves: u32 = 50;
	pub const MaxLocks: u32 = 50;
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
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type RuntimeHoldReason = ();
	type FreezeIdentifier = ();
	type RuntimeFreezeReason = ();
	// Holds are used with COLLATOR_LOCK_ID and DELEGATOR_LOCK_ID
	type MaxHolds = ConstU32<2>;
	type MaxFreezes = ConstU32<0>;
}

parameter_types! {
	pub const Epoch: BlockNumber = 5;
	pub const Treasury: AccountId = AccountId::new([7u8; 32]);
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Epoch = Epoch;
	type Treasury = Treasury;
}

pub fn events() -> Vec<RuntimeEvent> {
	let evt = System::events().into_iter().map(|evt| evt.event).collect::<Vec<_>>();

	System::reset_events();

	evt
}
