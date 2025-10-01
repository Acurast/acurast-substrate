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
	PalletId,
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
	pub const BusyWeightBonus: Perquintill = Perquintill::from_percent(20);
	pub const MetricEpochValidity: BlockNumber = 100;
	pub const WarmupPeriod: BlockNumber = 30;
	pub const MaxMetricCommitmentRatio: Perquintill = Perquintill::from_percent(80);
	pub const MinCooldownPeriod: BlockNumber = 36;
	pub const MaxCooldownPeriod: BlockNumber = 108;
	pub const TargetCooldownPeriod: BlockNumber = 72; // Target cooldown period for economic calculations
	pub const TargetStakedTokenSupply: Perquintill = Perquintill::from_percent(50); // Target 50% of total supply staked
	pub const MinDelegation: Balance = 1;
	pub const MaxDelegationRatio: Perquintill = Perquintill::from_percent(90);
	pub const CooldownRewardRatio: Perquintill = Perquintill::from_percent(50);
	pub const RedelegationBlockingPeriod: BlockNumber = 3; // can redelegate once per 3 epochs
	pub const MinStake: Balance = UNIT;

	pub const ComputeStakingLockId: LockIdentifier = *b"compstak";
	pub const ComputePalletId: PalletId = PalletId(*b"cmptepid");
	pub const InflationStakedComputeRation: Perquintill = Perquintill::from_percent(70);
	pub const InflationMetricsRation: Perquintill = Perquintill::from_percent(30);
	pub const TreasuryAccountId: AccountId = alice_account_id();
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type PalletId = ComputePalletId;
	type ManagerId = u128;
	type CommitmentId = u128;
	type ManagerIdProvider = AcurastManagerIdProvider;
	type CommitmentIdProvider = AcurastCommitmentIdProvider;
	type Epoch = Epoch;
	type BusyWeightBonus = BusyWeightBonus;
	type MaxPools = ConstU32<30>;
	type MaxMetricCommitmentRatio = MaxMetricCommitmentRatio;
	type MinCooldownPeriod = MinCooldownPeriod;
	type MaxCooldownPeriod = MaxCooldownPeriod;
	type TargetCooldownPeriod = TargetCooldownPeriod;
	type TargetStakedTokenSupply = TargetStakedTokenSupply;
	type MinDelegation = MinDelegation;
	type MaxDelegationRatio = MaxDelegationRatio;
	type CooldownRewardRatio = CooldownRewardRatio;
	type RedelegationBlockingPeriod = RedelegationBlockingPeriod;
	type MinStake = MinStake;
	type MetricValidity = MetricEpochValidity;
	type WarmupPeriod = WarmupPeriod;
	type Currency = Balances;
	type LockIdentifier = ComputeStakingLockId;
	type ManagerProviderForEligibleProcessor = MockManagerProvider<Self::AccountId>;
	type InflationPerEpoch = InflationPerEpoch;
	type InflationStakedComputeRation = InflationStakedComputeRation;
	type InflationMetricsRation = InflationMetricsRation;
	type InflationHandler = ();
	type CreateModifyPoolOrigin = EnsureRoot<Self::AccountId>;
	type OperatorOrigin = EnsureRoot<Self::AccountId>;
	type WeightInfo = ();
}

use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
	static MANAGER_MAPPINGS: RefCell<HashMap<AccountId32, AccountId32>> = RefCell::new({
		let mut map = HashMap::new();
		map.insert(alice_account_id(), charlie_account_id());
		map.insert(bob_account_id(), charlie_account_id());
		map
	});

	static INFLATION_PER_EPOCH: RefCell<Balance> = RefCell::new(8_561_643_835_616_438);
}

/// Dynamic parameter type for InflationPerEpoch that can be modified in tests
pub struct InflationPerEpoch;
impl frame_support::traits::Get<Balance> for InflationPerEpoch {
	fn get() -> Balance {
		INFLATION_PER_EPOCH.with(|v| *v.borrow())
	}
}

/// Helper functions for managing inflation per epoch in tests
impl InflationPerEpoch {
	/// Set a custom inflation per epoch for tests
	pub fn set(value: Balance) {
		INFLATION_PER_EPOCH.with(|v| {
			*v.borrow_mut() = value;
		});
	}

	/// Reset to default value
	pub fn reset() {
		INFLATION_PER_EPOCH.with(|v| {
			*v.borrow_mut() = 1 * UNIT;
		});
	}
}

/// Mock manager provider with configurable mappings.
pub struct MockManagerProvider<AccountId>(PhantomData<AccountId>);

impl MockManagerProvider<AccountId32> {
	/// Set a custom processor -> manager mapping for tests
	pub fn set_mapping(processor: AccountId32, manager: AccountId32) {
		MANAGER_MAPPINGS.with(|mappings| {
			mappings.borrow_mut().insert(processor, manager);
		});
	}

	/// Clear all custom mappings
	pub fn clear_mappings() {
		MANAGER_MAPPINGS.with(|mappings| {
			mappings.borrow_mut().clear();
		});
	}
}

impl<AccountId: Clone + IsType<AccountId32>> AccountLookup<AccountId>
	for MockManagerProvider<AccountId>
{
	fn lookup(processor: &AccountId) -> Option<AccountId> {
		let processor_id: AccountId32 = processor.clone().into();

		// Check custom mappings first
		MANAGER_MAPPINGS
			.with(|mappings| mappings.borrow().get(&processor_id).cloned())
			.map(|a| a.into())
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
