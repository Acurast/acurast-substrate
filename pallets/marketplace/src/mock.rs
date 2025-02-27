#[cfg(feature = "runtime-benchmarks")]
use frame_support::traits::fungible;
use frame_support::{
	derive_impl, parameter_types,
	sp_runtime::{
		traits::{AccountIdConversion, IdentityLookup},
		BuildStorage, DispatchError, Percent,
	},
	PalletId,
};
use sp_core::*;
use sp_io;
use sp_std::prelude::*;

use pallet_acurast::{JobModules, CU32};

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
		AcurastMarketplace: crate::{Pallet, Call, Storage, Event<T>}
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
	type RegistrationExtra = ExtraFor<Test>;
	type PalletId = AcurastPalletId;
	type HyperdrivePalletId = HyperdrivePalletId;
	type ReportTolerance = ReportTolerance;
	type Balance = Balance;
	type ManagerProvider = ManagerOf;
	type RewardManager = AssetRewardManager<FeeManagerImpl, Balances, Pallet<Self>>;
	type ProcessorInfoProvider = ProcessorLastSeenProvider;
	type MarketplaceHooks = ();
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
