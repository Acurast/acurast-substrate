use std::marker::PhantomData;

use acurast_common::AccountLookup;
use frame_support::{
	derive_impl, parameter_types,
	sp_runtime::{
		traits::{ConstU128, ConstU32, IdentityLookup},
		BuildStorage,
	},
	traits::{AsEnsureOriginWithArg, ConstU16},
};
use frame_system::{EnsureRoot, EnsureRootWithSuccess};
use sp_core::H256;
use sp_std::prelude::*;

use crate::{stub::*, *};

type Block = frame_system::mocking::MockBlock<Test>;

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
		Balances: pallet_balances::{Pallet, Call, Config<T>, Storage, Event<T>},
		Uniques: pallet_uniques::{Pallet, Storage, Event<T>, Call},
		Compute: crate::{Pallet, Call, Config<T, I>, Storage, Event<T>},
	}
);

parameter_types! {
	pub const BlockHashCount: BlockNumber = 2400;
	pub const RootAccountId: AccountId = alice_account_id();
}

#[derive_impl(frame_system::config_preludes::ParaChainDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Nonce = u64;
	type Hash = H256;
	type Block = Block;
	type BlockHashCount = BlockHashCount;
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
	pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
	pub const MaxLocks: u32 = 50;
	pub const MaxReserves: u32 = 50;
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
	type FreezeIdentifier = ();
	type RuntimeHoldReason = ();
	type RuntimeFreezeReason = ();
	type MaxFreezes = ConstU32<0>;
	type DoneSlashHandler = ();
}

impl pallet_uniques::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type CollectionId = u128;
	type ItemId = u128;
	type Currency = Balances;
	type ForceOrigin = EnsureRoot<Self::AccountId>;
	type CreateOrigin =
		AsEnsureOriginWithArg<EnsureRootWithSuccess<Self::AccountId, RootAccountId>>;
	type Locker = ();
	type CollectionDeposit = ConstU128<0>;
	type ItemDeposit = ConstU128<0>;
	type MetadataDepositBase = ConstU128<0>;
	type AttributeDepositBase = ConstU128<0>;
	type DepositPerByte = ConstU128<0>;
	type StringLimit = ConstU32<256>;
	type KeyLimit = ConstU32<256>;
	type ValueLimit = ConstU32<256>;
	type WeightInfo = pallet_uniques::weights::SubstrateWeight<Self>;
	#[cfg(feature = "runtime-benchmarks")]
	type Helper = ();
}

parameter_types! {
	pub const EpochBase: BlockNumber = 0;
	pub const Epoch: BlockNumber = 100;
	pub const MetricEpochValidity: BlockNumber = 100;
	pub const WarmupPeriod: BlockNumber = 30;
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type EpochBase = EpochBase;
	type Epoch = Epoch;
	type MetricValidity = MetricEpochValidity;
	type WarmupPeriod = WarmupPeriod;
	type Currency = Balances;
	type EligibleRewardAccountLookup = MockLookup<Self::AccountId>;
	type WeightInfo = ();
}

pub struct MockLookup<AccountId>(PhantomData<AccountId>);
impl<AccountId: Clone> AccountLookup<AccountId> for MockLookup<AccountId> {
	fn lookup(processor: &AccountId) -> Option<AccountId> {
		Some(processor.clone())
	}
}

pub fn events() -> Vec<RuntimeEvent> {
	let evt = System::events().into_iter().map(|evt| evt.event).collect::<Vec<_>>();

	System::reset_events();

	evt
}
