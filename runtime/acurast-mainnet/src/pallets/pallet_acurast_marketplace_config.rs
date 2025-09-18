use frame_support::{pallet_prelude::DispatchResultWithPostInfo, PalletId};
use frame_system::EnsureRoot;
use sp_core::{ConstU32, ConstU64};
use sp_runtime::{traits::BlakeTwo256, AccountId32, DispatchError};
use sp_std::prelude::*;

use acurast_runtime_common::{
	types::{AccountId, Balance, ExtraFor},
	weight,
};
use pallet_acurast::{JobId, MultiOrigin, CU32};
use pallet_acurast_hyperdrive::{IncomingAction, ProxyChain};
use pallet_acurast_marketplace::{MarketplaceHooks, PubKey, PubKeys};

#[cfg(feature = "runtime-benchmarks")]
use crate::benchmarking;
use crate::{
	AcurastCompute, AcurastHyperdrive, AcurastMarketplace, AcurastPalletId,
	AcurastProcessorManager, Balances, DefaultFeePercentage, DefaultMatcherFeePercentage,
	FeeManagerPalletId, HyperdriveIbcFeePalletAccount, HyperdrivePalletId, ReportTolerance,
	Runtime, RuntimeEvent,
};

/// Runtime configuration for pallet_acurast_marketplace.
impl pallet_acurast_marketplace::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MaxAllowedConsumers = CU32<100>;
	type Competing = CU32<4>;
	type MatchingCompetingMinInterval = ConstU64<300_000>; // 5 min
	type MatchingCompetingDueDelta = ConstU64<120_000>; // 2 min
	type MaxProposedMatches = ConstU32<10>;
	type MaxProposedExecutionMatches = ConstU32<10>;
	type MaxFinalizeJobs = ConstU32<10>;
	type MaxJobCleanups = ConstU32<100>;
	type RegistrationExtra = ExtraFor<Self>;
	type PalletId = AcurastPalletId;
	type HyperdrivePalletId = HyperdrivePalletId;
	type ReportTolerance = ReportTolerance;
	type Balance = Balance;
	type RewardManager =
		pallet_acurast_marketplace::AssetRewardManager<FeeManagement, Balances, AcurastMarketplace>;
	type ManagerProvider = AcurastProcessorManager;
	type ProcessorInfoProvider = ProcessorLastSeenProvider;
	type MarketplaceHooks = HyperdriveOutgoingMarketplaceHooks;
	type DeploymentHashing = BlakeTwo256;
	type KeyIdHashing = BlakeTwo256;
	type UpdateOrigin = EnsureRoot<Self::AccountId>;
	type OperatorOrigin = EnsureRoot<Self::AccountId>;
	type WeightInfo = weight::pallet_acurast_marketplace::WeightInfo<Self>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarking::AcurastBenchmarkHelper;
}

/// Reward fee management implementation.
pub struct FeeManagement;
impl pallet_acurast_marketplace::FeeManager for FeeManagement {
	fn get_fee_percentage() -> sp_runtime::Percent {
		DefaultFeePercentage::get()
		// AcurastFeeManager::fee_percentage(AcurastFeeManager::fee_version())
	}

	fn get_matcher_percentage() -> sp_runtime::Percent {
		DefaultMatcherFeePercentage::get()
		// AcurastMatcherFeeManager::fee_percentage(AcurastMatcherFeeManager::fee_version())
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

pub struct HyperdriveOutgoingMarketplaceHooks;
impl MarketplaceHooks<Runtime> for HyperdriveOutgoingMarketplaceHooks {
	fn assign_job(job_id: &JobId<AccountId32>, pub_keys: &PubKeys) -> DispatchResultWithPostInfo {
		// inspect which hyperdrive proxy chain to send action to
		let (origin, job_id_seq) = job_id;

		// depending on the origin=target chain to send message to, we search for a supported
		// processor public key supported on the target
		match origin {
			MultiOrigin::AlephZero(_) => {
				let key = pub_keys
					.iter()
					.find(|key| matches!(key, PubKey::SECP256k1(_)))
					.ok_or(DispatchError::Other("k256 public key does not exist"))?;

				AcurastHyperdrive::send_to_proxy(
					ProxyChain::AlephZero,
					IncomingAction::AssignJob(*job_id_seq, key.clone()),
					&HyperdriveIbcFeePalletAccount::get(),
				)?;

				Ok(().into())
			},
			MultiOrigin::Vara(_) => {
				let key = pub_keys
					.iter()
					.find(|key| matches!(key, PubKey::SECP256k1(_)))
					.ok_or(DispatchError::Other("k256 public key does not exist"))?;

				AcurastHyperdrive::send_to_proxy(
					ProxyChain::Vara,
					IncomingAction::AssignJob(*job_id_seq, key.clone()),
					&HyperdriveIbcFeePalletAccount::get(),
				)?;

				Ok(().into())
			},
			_ => Ok(().into()),
		}
	}

	fn finalize_job(
		job_id: &JobId<AccountId>,
		refund: <Runtime as pallet_acurast_marketplace::Config>::Balance,
	) -> DispatchResultWithPostInfo {
		// inspect which hyperdrive proxy chain to send action to
		let (origin, job_id_seq) = job_id;

		match origin {
			MultiOrigin::AlephZero(_) => {
				AcurastHyperdrive::send_to_proxy(
					ProxyChain::AlephZero,
					IncomingAction::FinalizeJob(*job_id_seq, refund),
					&HyperdriveIbcFeePalletAccount::get(),
				)?;

				Ok(().into())
			},
			MultiOrigin::Vara(_) => {
				AcurastHyperdrive::send_to_proxy(
					ProxyChain::Vara,
					IncomingAction::FinalizeJob(*job_id_seq, refund),
					&HyperdriveIbcFeePalletAccount::get(),
				)?;

				Ok(().into())
			},
			_ => Ok(().into()),
		}
	}
}
