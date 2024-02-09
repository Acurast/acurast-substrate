use frame_support::sp_runtime::traits::{AccountIdConversion, AccountIdLookup, BlakeTwo256};
use frame_support::sp_runtime::DispatchError;
use frame_support::sp_runtime::{BuildStorage, Percent};
use frame_support::{parameter_types, traits::Everything, PalletId};
use sp_core::*;
use sp_io;
use sp_std::prelude::*;

use pallet_acurast::{
    CertificateRevocationListUpdate, JobModules, RevocationListUpdateBarrier, CU32,
};

use crate::stub::*;
use crate::*;

type Block = frame_system::mocking::MockBlock<Test>;

pub struct Barrier;

impl RevocationListUpdateBarrier<Test> for Barrier {
    fn can_update_revocation_list(
        origin: &<Test as frame_system::Config>::AccountId,
        _updates: &Vec<CertificateRevocationListUpdate>,
    ) -> bool {
        AllowedRevocationListUpdate::get().contains(origin)
    }
}

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
        let mut t = frame_system::GenesisConfig::<Test>::default()
            .build_storage()
            .unwrap();

        let parachain_info_config = parachain_info::GenesisConfig {
            parachain_id: 2000.into(),
            ..Default::default()
        };

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
    pub const ReportTolerance: u64 = 12000;
}

impl frame_system::Config for Test {
    type RuntimeCall = RuntimeCall;
    type Nonce = u32;
    type Block = Block;
    type Hash = sp_core::H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = AccountIdLookup<AccountId, ()>;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type BlockHashCount = BlockHashCount;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type DbWeight = ();
    type BaseCallFilter = Everything;
    type SystemWeightInfo = ();
    type BlockWeights = ();
    type BlockLength = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
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
    // Holds are used with COLLATOR_LOCK_ID and DELEGATOR_LOCK_ID
    type MaxHolds = ConstU32<2>;
    type MaxFreezes = ConstU32<0>;
}

impl parachain_info::Config for Test {}

impl pallet_acurast::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type RegistrationExtra = JobRequirementsFor<Self>;
    type MaxAllowedSources = CU32<4>;
    type MaxCertificateRevocationListUpdates = frame_support::traits::ConstU32<10>;
    type MaxSlots = CU32<64>;
    type PalletId = AcurastPalletId;
    type MaxEnvVars = CU32<10>;
    type EnvKeyMaxSize = CU32<32>;
    type EnvValueMaxSize = CU32<1024>;
    type RevocationListUpdateBarrier = Barrier;
    type KeyAttestationBarrier = ();
    type UnixTime = pallet_timestamp::Pallet<Test>;
    type JobHooks = Pallet<Test>;
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
        JobRequirements {
            slots: 1,
            reward: 1,
            min_reputation: None,
            instant_match: None,
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

impl crate::traits::ProcessorLastSeenProvider<Test> for ProcessorLastSeenProvider {
    fn last_seen(_processor: &<Test as frame_system::Config>::AccountId) -> Option<u128> {
        Some(AcurastMarketplace::now().unwrap().into())
    }
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type MaxAllowedConsumers = pallet_acurast::CU32<4>;
    type MaxProposedMatches = frame_support::traits::ConstU32<10>;
    type MaxFinalizeJobs = frame_support::traits::ConstU32<10>;
    type RegistrationExtra = JobRequirementsFor<Self>;
    type PalletId = AcurastPalletId;
    type HyperdrivePalletId = HyperdrivePalletId;
    type ReportTolerance = ReportTolerance;
    type Balance = Balance;
    type ManagerProvider = ManagerOf;
    type RewardManager = AssetRewardManager<FeeManagerImpl, Balances, Pallet<Self>>;
    type ProcessorLastSeenProvider = ProcessorLastSeenProvider;
    type MarketplaceHooks = ();
    type WeightInfo = weights::WeightInfo<Test>;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = TestBenchmarkHelper;
}

#[cfg(feature = "runtime-benchmarks")]
impl crate::benchmarking::BenchmarkHelper<Test> for TestBenchmarkHelper {
    fn registration_extra(r: JobRequirementsFor<Test>) -> <Test as Config>::RegistrationExtra {
        r
    }

    fn funded_account(index: u32, amount: Balance) -> AccountId {
        let caller: AccountId = frame_benchmarking::account("token_account", index, SEED);
        <Balances as fungible::Mutate<_>>::set_balance(&caller, amount);

        caller
    }
}

pub fn events() -> Vec<RuntimeEvent> {
    let evt = System::events()
        .into_iter()
        .map(|evt| evt.event)
        .collect::<Vec<_>>();

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
