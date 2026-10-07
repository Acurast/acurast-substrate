use frame_support::PalletId;
use sp_core::{parameter_types, ConstU32, ConstU64};
use sp_runtime::{traits::BlakeTwo256, FixedU128};

use acurast_runtime_common::{
	types::{AcurastProcessorInfoProvider, Balance, ExtraFor, ProcessorPriceProvider},
	weight,
};
use pallet_acurast::CU32;

#[cfg(feature = "runtime-benchmarks")]
use crate::benchmarking;
use crate::{
	AcurastCompute, AcurastMarketplace, AcurastPalletId, Balances, DefaultFeePercentage,
	DefaultMatcherFeePercentage, EnsureCouncilOrRoot, FeeManagerPalletId, HyperdrivePalletId,
	ReportTolerance, Runtime,
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
	type MatchingCompetingDueDelta = ConstU64<300_000>; // 4 min
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
	type ProcessorInfoProvider = AcurastProcessorInfoProvider<Self>;
	// Cross-chain (Hyperdrive) job-operation hooks removed; no-op hooks.
	type MarketplaceHooks = ();
	type DeploymentHashing = BlakeTwo256;
	type KeyIdHashing = BlakeTwo256;
	type DefaultMinPrice = MinPrice;
	type DefaultPriceMultiplier = PriceMultiplier;
	type ProcessorPriceProvider = ProcessorPriceProvider<Self, AcurastCompute>;
	type UpdateOrigin = EnsureCouncilOrRoot;
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
