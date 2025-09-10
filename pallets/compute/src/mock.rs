use std::marker::PhantomData;

use acurast_common::{AccountLookup, CommitmentIdProvider, ManagerIdProvider};
use frame_support::{
	derive_impl, parameter_types,
	sp_runtime::{
		traits::{ConstU128, ConstU32, IdentityLookup},
		BuildStorage, Perquintill,
	},
	traits::{
		nonfungibles::{Create, InspectEnumerable as NFTInspectEnumerable},
		AsEnsureOriginWithArg, ConstU16, IsType, LockIdentifier,
	},
};
use frame_system::{EnsureRoot, EnsureRootWithSuccess};
use sp_core::H256;
use sp_runtime::AccountId32;
use sp_std::prelude::*;

use crate::{stub::*, *};

type Block = frame_system::mocking::MockBlock<Test>;

#[derive(Default)]
pub struct ExtBuilder;

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| {
			System::set_block_number(1);
		});
		ext
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
	pub const Epoch: BlockNumber = 100;
	pub const Era: BlockNumber = 3;
	pub const MetricEpochValidity: BlockNumber = 100;
	pub const WarmupPeriod: BlockNumber = 30;
	pub const MaxMetricCommitmentRatio: Perquintill = Perquintill::from_percent(80);
	pub const MinCooldownPeriod: BlockNumber = 36;
	pub const MaxCooldownPeriod: BlockNumber = 108;
	pub const MinDelegation: Balance = 1;
	pub const MaxDelegationRatio: Perquintill = Perquintill::from_percent(90);
	pub const CooldownRewardRatio: Perquintill = Perquintill::from_percent(50);
	pub const MinStake: Balance = 1 * UNIT;

	pub const ComputeStakingLockId: LockIdentifier = *b"compstak";
	pub const Decimals: Balance = UNIT;
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type ManagerId = u128;
	type CommitmentId = u128;
	type ManagerIdProvider = AcurastManagerIdProvider;
	type CommitmentIdProvider = AcurastCommitmentIdProvider;
	type Epoch = Epoch;
	type Era = Era;
	type MaxPools = ConstU32<30>;
	type MaxMetricCommitmentRatio = MaxMetricCommitmentRatio;
	type MinCooldownPeriod = MinCooldownPeriod;
	type MaxCooldownPeriod = MaxCooldownPeriod;
	type MinDelegation = MinDelegation;
	type MaxDelegationRatio = MaxDelegationRatio;
	type CooldownRewardRatio = CooldownRewardRatio;
	type MinStake = MinStake;
	type MetricValidity = MetricEpochValidity;
	type WarmupPeriod = WarmupPeriod;
	type Currency = Balances;
	type Decimals = Decimals;
	type LockIdentifier = ComputeStakingLockId;
	type ManagerProviderForEligibleProcessor = MockManagerProvider<Self::AccountId>;
	type WeightInfo = ();
}

/// Mock manager provider that returns manager charlie for processors alice and bob.
pub struct MockManagerProvider<AccountId>(PhantomData<AccountId>);
impl<AccountId: Clone + IsType<AccountId32>> AccountLookup<AccountId>
	for MockManagerProvider<AccountId>
{
	fn lookup(processor: &AccountId) -> Option<AccountId> {
		const ALICE: AccountId32 = alice_account_id();
		const BOB: AccountId32 = bob_account_id();
		match processor.clone().into() {
			ALICE | BOB => Some(charlie_account_id().into()),
			_ => None,
		}
	}
}

pub const MANAGER_COLLECTION_ID: u128 = 0;
pub const COMMITMENT_COLLECTION_ID: u128 = 1;

pub struct AcurastManagerIdProvider;
impl ManagerIdProvider<<Test as frame_system::Config>::AccountId, <Test as Config>::ManagerId>
	for AcurastManagerIdProvider
{
	fn create_manager_id(
		id: <Test as Config>::ManagerId,
		owner: &<Test as frame_system::Config>::AccountId,
	) -> frame_support::pallet_prelude::DispatchResult {
		if Uniques::collection_owner(MANAGER_COLLECTION_ID).is_none() {
			Uniques::create_collection(
				&MANAGER_COLLECTION_ID,
				&alice_account_id(),
				&alice_account_id(),
			)?;
		}
		Uniques::do_mint(MANAGER_COLLECTION_ID, id, owner.clone(), |_| Ok(()))
	}

	fn manager_id_for(
		owner: &<Test as frame_system::Config>::AccountId,
	) -> Result<<Test as Config>::ManagerId, frame_support::sp_runtime::DispatchError> {
		Uniques::owned_in_collection(&MANAGER_COLLECTION_ID, owner)
			.nth(0)
			.ok_or(frame_support::pallet_prelude::DispatchError::Other("Manager ID not found"))
	}

	fn owner_for(
		manager_id: <Test as Config>::ManagerId,
	) -> Result<<Test as frame_system::Config>::AccountId, frame_support::sp_runtime::DispatchError>
	{
		Uniques::owner(MANAGER_COLLECTION_ID, manager_id).ok_or(
			frame_support::pallet_prelude::DispatchError::Other(
				"Onwer for provided Manager ID not found",
			),
		)
	}
}

pub struct AcurastCommitmentIdProvider;
impl CommitmentIdProvider<<Test as frame_system::Config>::AccountId, <Test as Config>::CommitmentId>
	for AcurastCommitmentIdProvider
{
	fn create_commitment_id(
		id: <Test as Config>::CommitmentId,
		owner: &<Test as frame_system::Config>::AccountId,
	) -> frame_support::pallet_prelude::DispatchResult {
		if Uniques::collection_owner(COMMITMENT_COLLECTION_ID).is_none() {
			Uniques::create_collection(
				&COMMITMENT_COLLECTION_ID,
				&alice_account_id(),
				&alice_account_id(),
			)?;
		}
		Uniques::do_mint(COMMITMENT_COLLECTION_ID, id, owner.clone(), |_| Ok(()))
	}

	fn commitment_id_for(
		owner: &<Test as frame_system::Config>::AccountId,
	) -> Result<<Test as Config>::CommitmentId, frame_support::sp_runtime::DispatchError> {
		Uniques::owned_in_collection(&COMMITMENT_COLLECTION_ID, owner)
			.nth(0)
			.ok_or(frame_support::pallet_prelude::DispatchError::Other("Commitment ID not found"))
	}

	fn owner_for(
		commitment_id: <Test as Config>::CommitmentId,
	) -> Result<<Test as frame_system::Config>::AccountId, frame_support::sp_runtime::DispatchError>
	{
		Uniques::owner(COMMITMENT_COLLECTION_ID, commitment_id).ok_or(
			frame_support::pallet_prelude::DispatchError::Other(
				"Onwer for provided Commitment ID not found",
			),
		)
	}
}

pub fn events() -> Vec<RuntimeEvent> {
	let evt = System::events().into_iter().map(|evt| evt.event).collect::<Vec<_>>();

	System::reset_events();

	evt
}
