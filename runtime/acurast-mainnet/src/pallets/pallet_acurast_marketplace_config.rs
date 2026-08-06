use frame_support::{parameter_types, PalletId};
use frame_system::EnsureRoot;
use sp_core::{ConstU32, ConstU64};
use sp_runtime::{traits::BlakeTwo256, FixedU128};

use acurast_runtime_common::{
	types::{Balance, ExtraFor, ProcessorPriceProvider},
	weight,
};
use pallet_acurast::CU32;

#[cfg(feature = "runtime-benchmarks")]
use crate::benchmarking;
use crate::{
	AcurastCompute, AcurastMarketplace, AcurastPalletId, AcurastProcessorManager, Balances,
	DefaultFeePercentage, DefaultMatcherFeePercentage, EnsureCouncilOrRoot, FeeManagerPalletId,
	HyperdrivePalletId, ReportTolerance, Runtime,
};

parameter_types! {
	pub const MinPrice: Balance = 2_000_000_000;
	pub const PriceMultiplier: FixedU128 = FixedU128::from_rational(143, 100);
}

/// Runtime configuration for pallet_acurast_marketplace.
impl pallet_acurast_marketplace::Config for Runtime {
	type MaxAllowedConsumers = CU32<100>;
	type Competing = CU32<4>;
	type MatchingCompetingMinInterval = ConstU64<300_000>; // 5 min
	type MatchingCompetingDueDelta = ConstU64<300_000>; // 5 min
	type MaxProposedMatches = ConstU32<3>;
	type MaxProposedExecutionMatches = ConstU32<3>;
	type MaxFinalizeJobs = ConstU32<10>;
	type MaxJobCleanups = ConstU32<100>;
	type MaxMatchesPerProcessor = ConstU32<5>;
	type MinDuration = ConstU64<60_000>; // 1 min
	type MaxStartWindow = ConstU64<86_400_000>; // 24 h
	type MaxStartDelay = ConstU64<3_600_000>; // 1 h
	type RegistrationExtra = ExtraFor<Self>;
	type PalletId = AcurastPalletId;
	type HyperdrivePalletId = HyperdrivePalletId;
	type ReportTolerance = ReportTolerance;
	type Balance = Balance;
	type RewardManager = pallet_acurast_marketplace::AssetRewardManager<
		FeeManagement,
		Balances,
		AcurastMarketplace,
		(),
	>;
	type ProcessorInfoProvider = ProcessorLastSeenProvider;
	// Cross-chain (Hyperdrive) job-operation hooks removed; no-op hooks.
	type MarketplaceHooks = ();
	type DeploymentHashing = BlakeTwo256;
	type KeyIdHashing = BlakeTwo256;
	type DefaultMinPrice = MinPrice;
	type DefaultPriceMultiplier = PriceMultiplier;
	type ProcessorPriceProvider = ProcessorPriceProvider<Self, AcurastCompute>;
	type UpdateOrigin = EnsureRoot<Self::AccountId>;
	type OperatorOrigin = EnsureCouncilOrRoot;
	type WeightInfo = weight::pallet_acurast_marketplace::WeightInfo<Self>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarking::AcurastBenchmarkHelper;
}

/// Reward fee management implementation.
pub struct FeeManagement;
impl pallet_acurast_marketplace::FeeManager for FeeManagement {
	fn get_fee_percentage() -> sp_runtime::Percent {
		DefaultFeePercentage::get()
	}

	fn get_matcher_percentage() -> sp_runtime::Percent {
		DefaultMatcherFeePercentage::get()
	}

	fn pallet_id() -> PalletId {
		FeeManagerPalletId::get()
	}
}

pub struct ProcessorLastSeenProvider;
impl pallet_acurast_marketplace::traits::ProcessorInfoProvider<Runtime>
	for ProcessorLastSeenProvider
{
	fn last_seen(processor: &<Runtime as frame_system::Config>::AccountId) -> Option<u128> {
		AcurastProcessorManager::processor_last_seen(processor)
	}

	fn processor_version(
		processor: &<Runtime as frame_system::Config>::AccountId,
	) -> Option<<Runtime as pallet_acurast::Config>::ProcessorVersion> {
		AcurastProcessorManager::processor_version(processor)
	}

	fn last_processor_metric(
		processor: &<Runtime as frame_system::Config>::AccountId,
		pool_id: pallet_acurast::PoolId,
	) -> Option<sp_runtime::FixedU128> {
		let metric = AcurastCompute::metrics(&processor, pool_id)?;
		Some(metric.metric)
	}
}
