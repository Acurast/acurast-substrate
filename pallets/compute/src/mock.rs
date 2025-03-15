use std::marker::PhantomData;

use acurast_common::ManagerIdProvider;
use frame_support::{
	derive_impl,
	dispatch::DispatchResult,
	parameter_types,
	sp_runtime::{
		traits::{ConstU128, ConstU32, IdentityLookup},
		BuildStorage,
	},
	traits::{
		nonfungibles::{Create, InspectEnumerable as NFTInspectEnumerable},
		AsEnsureOriginWithArg, ConstU16,
	},
	weights::WeightMeter,
};
use frame_system::{EnsureRoot, EnsureRootWithSuccess};
use sp_core::H256;
use sp_runtime::{DispatchError, Perquintill};
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
		MockPallet: mock_pallet::{Pallet, Event<T>}
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
	pub const WarmupPeriod: BlockNumber = 30;
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type ManagerId = AssetId;
	type ManagerIdProvider = AcurastManagerIdProvider;
	type EpochBase = EpochBase;
	type Epoch = Epoch;
	type WarmupPeriod = WarmupPeriod;
	type Balance = Balance;
	type BlockNumber = BlockNumber;
	type Currency = Balances;
	type ComputeRewardDistributor = MockComputeRewardDistributor<Self, ()>;
	type WeightInfo = ();
}

impl mock_pallet::Config for Test {
	type RuntimeEvent = RuntimeEvent;
}

pub struct AcurastManagerIdProvider;
impl ManagerIdProvider<<Test as frame_system::Config>::AccountId, <Test as Config>::ManagerId>
	for AcurastManagerIdProvider
{
	fn create_manager_id(
		id: <Test as Config>::ManagerId,
		owner: &<Test as frame_system::Config>::AccountId,
	) -> frame_support::pallet_prelude::DispatchResult {
		if Uniques::collection_owner(0).is_none() {
			Uniques::create_collection(&0, &alice_account_id(), &alice_account_id())?;
		}
		Uniques::do_mint(0, id, owner.clone(), |_| Ok(()))
	}

	fn manager_id_for(
		owner: &<Test as frame_system::Config>::AccountId,
	) -> Result<<Test as Config>::ManagerId, frame_support::sp_runtime::DispatchError> {
		Uniques::owned_in_collection(&0, owner)
			.nth(0)
			.ok_or(frame_support::pallet_prelude::DispatchError::Other("Manager ID not found"))
	}

	fn owner_for(
		manager_id: <Test as Config>::ManagerId,
	) -> Result<<Test as frame_system::Config>::AccountId, frame_support::sp_runtime::DispatchError>
	{
		Uniques::owner(0, manager_id).ok_or(frame_support::pallet_prelude::DispatchError::Other(
			"Onwer for provided Manager ID not found",
		))
	}
}

#[frame_support::pallet]
pub mod mock_pallet {
	use crate::EpochOf;
	use frame_support::pallet_prelude::*;
	use sp_runtime::Perquintill;

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config + crate::Config<I> {
		type RuntimeEvent: From<Event<Self, I>>
			+ IsType<<Self as frame_system::Config>::RuntimeEvent>;
	}

	#[pallet::pallet]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		CalculateReward(Perquintill, EpochOf<T, I>),
		DistributeReward(T::AccountId, T::Balance),
		IsElegibleForReward(T::AccountId),
	}
}

pub struct MockComputeRewardDistributor<T, I>(PhantomData<(T, I)>);

impl<T: Config<I> + mock_pallet::Config<I>, I: 'static> ComputeRewardDistributor<T, I>
	for MockComputeRewardDistributor<T, I>
where
	T::Balance: From<u64>,
{
	fn calculate_reward(
		ratio: Perquintill,
		epoch: EpochOf<T, I>,
	) -> Result<<T as Config<I>>::Balance, DispatchError> {
		mock_pallet::Pallet::deposit_event(mock_pallet::Event::<T, I>::CalculateReward(
			ratio, epoch,
		));
		Ok(ratio.mul_floor(UNIT.into()))
	}

	fn distribute_reward(
		processor: &T::AccountId,
		amount: <T as Config<I>>::Balance,
		_meter: &mut WeightMeter,
	) -> DispatchResult {
		mock_pallet::Pallet::deposit_event(mock_pallet::Event::<T, I>::DistributeReward(
			processor.clone(),
			amount,
		));
		Ok(())
	}

	fn is_elegible_for_reward(processor: &T::AccountId) -> bool {
		mock_pallet::Pallet::deposit_event(mock_pallet::Event::<T, I>::IsElegibleForReward(
			processor.clone(),
		));
		true
	}
}

pub fn events() -> Vec<RuntimeEvent> {
	let evt = System::events().into_iter().map(|evt| evt.event).collect::<Vec<_>>();

	System::reset_events();

	evt
}
