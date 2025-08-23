use core::marker::PhantomData;

#[cfg(feature = "runtime-benchmarks")]
use frame_support::traits::fungible;
use frame_support::{
	derive_impl, parameter_types,
	sp_runtime::{
		traits::{AccountIdConversion, BlakeTwo256, IdentityLookup},
		BuildStorage, DispatchError, Percent,
	},
	traits::AsEnsureOriginWithArg,
	PalletId,
};
use frame_system::{EnsureRoot, EnsureRootWithSuccess};
use sp_core::*;
use sp_io;
use sp_std::prelude::*;

use pallet_acurast::{AccountLookup, JobModules, CU32};

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
	pub const MetricEpochValidity: BlockNumber = 10;
	pub const WarmupPeriod: BlockNumber = 1800; // 3 hours, only for testing, we should use something like 2 weeks = 219027
}

impl pallet_acurast_compute::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Epoch = Epoch;
	type EpochBase = EpochBase;
	type MetricEpochValidity = MetricEpochValidity;
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

impl AccountLookup<<Test as frame_system::Config>::AccountId> for ManagerOf {
	fn lookup(
		processor: &<Test as frame_system::Config>::AccountId,
	) -> Option<<Test as frame_system::Config>::AccountId> {
		Some(processor.clone())
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
