use core::marker::PhantomData;

#[cfg(feature = "runtime-benchmarks")]
use frame_support::traits::fungible;
use frame_support::{
	derive_impl, parameter_types,
	sp_runtime::{
		traits::{AccountIdConversion, BlakeTwo256, IdentityLookup},
		BuildStorage, DispatchError, DispatchResult, Percent, Perquintill,
	},
	traits::{
		nonfungibles::{Create, InspectEnumerable},
		AsEnsureOriginWithArg,
	},
	PalletId,
};
use frame_system::{EnsureRoot, EnsureRootWithSuccess};
use pallet_acurast_compute::{ComputeRewardDistributor, EpochOf};
use sp_core::*;
use sp_io;
use sp_std::prelude::*;

use pallet_acurast::{JobModules, ManagerIdProvider, CU32};

use crate::{stub::*, *};

type Block = frame_system::mocking::MockBlock<Test>;

pub struct FeeManagerImpl;

impl FeeManager for FeeManagerImpl {
	fn get_fee_percentage() -> Percent {
		Percent::from_percent(30)
	}

	fn get_matcher_percentage() -> Percent {
		Percent::from_percent(10)
	}

	fn pallet_id() -> PalletId {
		PalletId(*b"acurfees")
	}
}

pub struct ExtBuilder;

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		let parachain_info_config =
			parachain_info::GenesisConfig { parachain_id: 2000.into(), ..Default::default() };

		<parachain_info::GenesisConfig<Test> as BuildStorage>::assimilate_storage(
			&parachain_info_config,
			&mut t,
		)
		.unwrap();

		pallet_balances::GenesisConfig::<Test> {
			balances: vec![
				(alice_account_id(), 100_000_000),
				(charlie_account_id(), 100_000_000),
				(pallet_fees_account(), INITIAL_BALANCE),
				(pallet_acurast_acount(), INITIAL_BALANCE),
				(bob_account_id(), INITIAL_BALANCE),
				(processor_account_id(), INITIAL_BALANCE),
				(processor_2_account_id(), INITIAL_BALANCE),
			],
			dev_accounts: None,
		}
		.assimilate_storage(&mut t)
		.unwrap();

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
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		ParachainInfo: parachain_info::{Pallet, Storage, Config<T>},
		Acurast: pallet_acurast::{Pallet, Call, Storage, Event<T>},
		AcurastMarketplace: crate::{Pallet, Call, Storage, Event<T>},
		AcurastCompute: pallet_acurast_compute::{Pallet, Call, Storage, Event<T>},
		Uniques: pallet_uniques,
		MockPallet: mock_pallet::{Pallet, Event<T>},
	}
);

parameter_types! {
	pub const BlockHashCount: BlockNumber = 2400;
}
parameter_types! {
	pub const MinimumPeriod: u64 = 2000;
	pub AllowedRevocationListUpdate: Vec<AccountId> = vec![alice_account_id(), <Test as crate::Config>::PalletId::get().into_account_truncating()];
	pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
}
parameter_types! {
	pub const MaxReserves: u32 = 50;
	pub const MaxLocks: u32 = 50;
	pub const AcurastPalletId: PalletId = PalletId(*b"acrstpid");
	pub const HyperdrivePalletId: PalletId = PalletId(*b"hypdrpid");
	pub const ReportTolerance: u64 = 70_000;
	pub RootAccountId: AccountId = alice_account_id();
}

#[derive_impl(frame_system::config_preludes::ParaChainDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Nonce = u64;
	type Hash = H256;
	type Block = Block;
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

impl pallet_timestamp::Config for Test {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
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

impl parachain_info::Config for Test {}

impl pallet_acurast::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RegistrationExtra = ExtraFor<Self>;
	type MaxAllowedSources = CU32<4>;
	type MaxCertificateRevocationListUpdates = frame_support::traits::ConstU32<10>;
	type MaxSlots = CU32<64>;
	type PalletId = AcurastPalletId;
	type MaxEnvVars = CU32<10>;
	type EnvKeyMaxSize = CU32<32>;
	type EnvValueMaxSize = CU32<1024>;
	type KeyAttestationBarrier = ();
	type UnixTime = pallet_timestamp::Pallet<Test>;
	type JobHooks = Pallet<Test>;
	type ProcessorVersion = u32;
	type MaxVersions = pallet_acurast::CU32<1>;
	type WeightInfo = pallet_acurast::weights::WeightInfo<Test>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = TestBenchmarkHelper;
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
	pub const Epoch: BlockNumber = 900; // 1.5 hours
	pub const WarmupPeriod: BlockNumber = 1800; // 3 hours, only for testing, we should use something like 2 weeks = 219027
}

impl pallet_acurast_compute::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type ManagerId = u128;
	type ManagerIdProvider = AcurastManagerIdProvider;
	type Epoch = Epoch;
	type EpochBase = EpochBase;
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
impl
	ManagerIdProvider<
		<Test as frame_system::Config>::AccountId,
		<Test as pallet_acurast_compute::Config>::ManagerId,
	> for AcurastManagerIdProvider
{
	fn create_manager_id(
		id: <Test as pallet_acurast_compute::Config>::ManagerId,
		owner: &<Test as frame_system::Config>::AccountId,
	) -> frame_support::pallet_prelude::DispatchResult {
		if Uniques::collection_owner(0).is_none() {
			Uniques::create_collection(&0, &alice_account_id(), &alice_account_id())?;
		}
		Uniques::do_mint(0, id, owner.clone(), |_| Ok(()))
	}

	fn manager_id_for(
		owner: &<Test as frame_system::Config>::AccountId,
	) -> Result<
		<Test as pallet_acurast_compute::Config>::ManagerId,
		frame_support::sp_runtime::DispatchError,
	> {
		Uniques::owned_in_collection(&0, owner)
			.nth(0)
			.ok_or(frame_support::pallet_prelude::DispatchError::Other("Manager ID not found"))
	}

	fn owner_for(
		manager_id: <Test as pallet_acurast_compute::Config>::ManagerId,
	) -> Result<<Test as frame_system::Config>::AccountId, frame_support::sp_runtime::DispatchError>
	{
		Uniques::owner(0, manager_id).ok_or(frame_support::pallet_prelude::DispatchError::Other(
			"Onwer for provided Manager ID not found",
		))
	}
}

pub struct MockComputeRewardDistributor<T, I>(PhantomData<(T, I)>);

impl<T: pallet_acurast_compute::Config<I> + mock_pallet::Config<I>, I: 'static>
	ComputeRewardDistributor<T, I> for MockComputeRewardDistributor<T, I>
where
	T::Balance: From<u64>,
{
	fn calculate_reward(
		ratio: Perquintill,
		epoch: EpochOf<T, I>,
	) -> Result<<T as pallet_acurast_compute::Config<I>>::Balance, DispatchError> {
		mock_pallet::Pallet::deposit_event(mock_pallet::Event::<T, I>::CalculateReward(
			ratio, epoch,
		));
		Ok(ratio.mul_floor(UNIT.into()))
	}

	fn distribute_reward(
		processor: &T::AccountId,
		amount: <T as pallet_acurast_compute::Config<I>>::Balance,
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

#[frame_support::pallet]
pub mod mock_pallet {
	use frame_support::{pallet_prelude::*, sp_runtime::Perquintill};
	use pallet_acurast_compute::EpochOf;

	#[pallet::config]
	pub trait Config<I: 'static = ()>:
		frame_system::Config + pallet_acurast_compute::Config<I>
	{
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

#[cfg(feature = "runtime-benchmarks")]
pub struct TestBenchmarkHelper;
#[cfg(feature = "runtime-benchmarks")]
impl pallet_acurast::BenchmarkHelper<Test> for TestBenchmarkHelper {
	fn registration_extra(
		_instant_match: bool,
	) -> <Test as pallet_acurast::Config>::RegistrationExtra {
		ExtraFor::<Test> {
			requirements: JobRequirements {
				assignment_strategy: AssignmentStrategy::Single(None),
				slots: 1,
				reward: 1,
				min_reputation: None,
				processor_version: Some(ProcessorVersionRequirements::Min(bounded_vec!(1))),
				runtime: Runtime::NodeJS,
			},
		}
	}

	fn funded_account(index: u32) -> AccountId {
		let caller: AccountId = frame_benchmarking::account("token_account", index, SEED);
		<Balances as fungible::Mutate<_>>::set_balance(&caller, u32::MAX.into());

		caller
	}
}

pub struct ManagerOf;

impl ManagerProvider<Test> for ManagerOf {
	fn manager_of(
		owner: &<Test as frame_system::Config>::AccountId,
	) -> Result<<Test as frame_system::Config>::AccountId, DispatchError> {
		Ok(owner.clone())
	}
}

pub struct ProcessorLastSeenProvider;

impl crate::traits::ProcessorInfoProvider<Test> for ProcessorLastSeenProvider {
	fn last_seen(_processor: &<Test as frame_system::Config>::AccountId) -> Option<u128> {
		Some(AcurastMarketplace::now().unwrap().into())
	}

	fn processor_version(
		_processor: &<Test as frame_system::Config>::AccountId,
	) -> Option<<Test as pallet_acurast::Config>::ProcessorVersion> {
		Some(1)
	}

	fn last_processor_metric(
		processor: &<Test as frame_system::Config>::AccountId,
		pool_id: pallet_acurast::PoolId,
	) -> Option<frame_support::sp_runtime::FixedU128> {
		Some(AcurastCompute::metrics(processor, pool_id)?.metric)
	}
}

type MaxSlotsFor<T> = <T as pallet_acurast::Config>::MaxSlots;
pub type ProcessorVersionFor<T> = <T as pallet_acurast::Config>::ProcessorVersion;
pub type MaxVersionsFor<T> = <T as pallet_acurast::Config>::MaxVersions;
pub type ExtraFor<T> = RegistrationExtra<
	Balance,
	AccountId,
	MaxSlotsFor<T>,
	ProcessorVersionFor<T>,
	MaxVersionsFor<T>,
>;

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MaxAllowedConsumers = pallet_acurast::CU32<4>;
	type Competing = pallet_acurast::CU32<2>;
	type MatchingCompetingMinInterval = frame_support::traits::ConstU64<300_000>;
	type MatchingCompetingDueDelta = frame_support::traits::ConstU64<120_000>;
	type MaxProposedMatches = frame_support::traits::ConstU32<10>;
	type MaxProposedExecutionMatches = frame_support::traits::ConstU32<10>;
	type MaxFinalizeJobs = frame_support::traits::ConstU32<10>;
	type MaxJobCleanups = frame_support::traits::ConstU32<100>;
	type RegistrationExtra = ExtraFor<Test>;
	type PalletId = AcurastPalletId;
	type HyperdrivePalletId = HyperdrivePalletId;
	type ReportTolerance = ReportTolerance;
	type Balance = Balance;
	type ManagerProvider = ManagerOf;
	type RewardManager = AssetRewardManager<FeeManagerImpl, Balances, Pallet<Self>>;
	type ProcessorInfoProvider = ProcessorLastSeenProvider;
	type MarketplaceHooks = ();
	type DeploymentHashing = BlakeTwo256;
	type KeyIdHashing = BlakeTwo256;
	type WeightInfo = weights::WeightInfo<Test>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = TestBenchmarkHelper;
}

#[cfg(feature = "runtime-benchmarks")]
impl crate::benchmarking::BenchmarkHelper<Test> for TestBenchmarkHelper {
	fn registration_extra(r: JobRequirementsFor<Test>) -> <Test as Config>::RegistrationExtra {
		ExtraFor::<Test> { requirements: r }
	}

	fn funded_account(index: u32, amount: Balance) -> AccountId {
		let caller: AccountId = frame_benchmarking::account("token_account", index, SEED);
		<Balances as fungible::Mutate<_>>::set_balance(&caller, amount);

		caller
	}

	fn remove_job_registration(job_id: &JobId<T::AccountId>) {}
}

pub fn events() -> Vec<RuntimeEvent> {
	let evt = System::events().into_iter().map(|evt| evt.event).collect::<Vec<_>>();

	System::reset_events();

	evt
}

pub fn pallet_fees_account() -> <Test as frame_system::Config>::AccountId {
	FeeManagerImpl::pallet_id().into_account_truncating()
}

pub fn pallet_acurast_acount() -> <Test as frame_system::Config>::AccountId {
	PalletId(*b"acrstpid").into_account_truncating()
}

pub fn advertisement(
	fee_per_millisecond: u128,
	fee_per_storage_byte: u128,
	storage_capacity: u32,
	max_memory: u32,
	network_request_quota: u8,
) -> AdvertisementFor<Test> {
	Advertisement {
		pricing: Pricing {
			fee_per_millisecond,
			fee_per_storage_byte,
			base_fee_per_execution: 0,
			scheduling_window: SchedulingWindow::Delta(2_628_000_000), // 1 month
		},
		allowed_consumers: None,
		storage_capacity,
		max_memory,
		network_request_quota,
		available_modules: JobModules::default(),
	}
}
