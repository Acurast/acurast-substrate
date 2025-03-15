use acurast_runtime_common::{
	types::{AccountId, Balance, ExtraFor},
	weight,
};
use frame_support::{pallet_prelude::DispatchResultWithPostInfo, weights::WeightMeter, PalletId};
use pallet_acurast::{JobId, MultiOrigin, CU32};
use pallet_acurast_hyperdrive::{IncomingAction, ProxyChain};
use pallet_acurast_marketplace::{MarketplaceHooks, PubKey, PubKeys};
use sp_core::{ConstU32, ConstU64};
use sp_runtime::{AccountId32, DispatchError};
use sp_std::prelude::*;

#[cfg(feature = "runtime-benchmarks")]
use crate::benchmarking;
use crate::{
	AcurastHyperdrive, AcurastMarketplace, AcurastPalletId, AcurastProcessorManager, Balances,
	DefaultFeePercentage, DefaultMatcherFeePercentage, FeeManagerPalletId,
	HyperdriveIbcFeePalletAccount, HyperdrivePalletId, ReportTolerance, Runtime, RuntimeEvent,
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
	type RegistrationExtra = ExtraFor<Self>;
	type PalletId = AcurastPalletId;
	type HyperdrivePalletId = HyperdrivePalletId;
	type ReportTolerance = ReportTolerance;
	type Balance = Balance;
	type RewardManager =
		pallet_acurast_marketplace::AssetRewardManager<FeeManagement, Balances, AcurastMarketplace>;
	type ManagerProvider = ManagerProvider;
	type ProcessorInfoProvider = ProcessorLastSeenProvider;
	type MarketplaceHooks = HyperdriveOutgoingMarketplaceHooks;
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

pub struct ManagerProvider;
impl pallet_acurast_marketplace::traits::ManagerProvider<Runtime> for ManagerProvider {
	fn manager_of(
		processor: &<Runtime as frame_system::Config>::AccountId,
	) -> Result<<Runtime as frame_system::Config>::AccountId, DispatchError> {
		match AcurastProcessorManager::manager_for_processor(processor, &mut WeightMeter::new()) {
			Some(manager) => Ok(manager),
			None => Err(DispatchError::Other("Processor without manager.")),
		}
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
}

pub struct HyperdriveOutgoingMarketplaceHooks;
impl MarketplaceHooks<Runtime> for HyperdriveOutgoingMarketplaceHooks {
	fn assign_job(job_id: &JobId<AccountId32>, pub_keys: &PubKeys) -> DispatchResultWithPostInfo {
		// inspect which hyperdrive proxy chain to send action to
		let (origin, job_id_seq) = job_id;

		// depending on the origin=target chain to send message to, we search for a supported
		// processor public key supported on the target
		match origin {
			MultiOrigin::Acurast(_) => Ok(().into()), // nothing to be done for Acurast
			MultiOrigin::Tezos(_) => {
				// TODO: reenable
				// let key = pub_keys
				// 	.iter()
				// 	.find(|key| match key {
				// 		PubKey::SECP256r1(_) => true,
				// 		_ => false,
				// 	})
				// 	.ok_or_else(|| DispatchError::Other("p256 public key does not exist"))?;

				// AcurastHyperdrive::send_to_proxy(Action::AssignJob(
				// 	job_id_seq.clone(),
				// 	key.clone(),
				// ))
				// .map_err(|_| DispatchError::Other("Could not send ASSIGN_JOB to tezos").into())

				Ok(().into())
			},
			MultiOrigin::Ethereum(_) => {
				// TODO: reenable
				// let key = pub_keys
				// 	.iter()
				// 	.find(|key| match key {
				// 		PubKey::SECP256k1(_) => true,
				// 		_ => false,
				// 	})
				// 	.ok_or_else(|| DispatchError::Other("k256 public key does not exist"))?;

				// AcurastHyperdrive::send_to_proxy(Action::AssignJob(
				// 	job_id_seq.clone(),
				// 	key.clone(),
				// ))
				// .map_err(|_| DispatchError::Other("Could not send ASSIGN_JOB to ethereum").into())

				Ok(().into())
			},
			MultiOrigin::AlephZero(_) => {
				let key = pub_keys
					.iter()
					.find(|key| match key {
						PubKey::SECP256k1(_) => true,
						_ => false,
					})
					.ok_or_else(|| DispatchError::Other("k256 public key does not exist"))?;

				AcurastHyperdrive::send_to_proxy(
					ProxyChain::AlephZero,
					IncomingAction::AssignJob(job_id_seq.clone(), key.clone()),
					&HyperdriveIbcFeePalletAccount::get(),
				)?;

				Ok(().into())
			},
			MultiOrigin::Vara(_) => {
				let key = pub_keys
					.iter()
					.find(|key| match key {
						PubKey::SECP256k1(_) => true,
						_ => false,
					})
					.ok_or_else(|| DispatchError::Other("k256 public key does not exist"))?;

				AcurastHyperdrive::send_to_proxy(
					ProxyChain::Vara,
					IncomingAction::AssignJob(job_id_seq.clone(), key.clone()),
					&HyperdriveIbcFeePalletAccount::get(),
				)?;

				Ok(().into())
			},
		}
	}

	fn finalize_job(
		job_id: &JobId<AccountId>,
		refund: <Runtime as pallet_acurast_marketplace::Config>::Balance,
	) -> DispatchResultWithPostInfo {
		// inspect which hyperdrive proxy chain to send action to
		let (origin, job_id_seq) = job_id;

		match origin {
			MultiOrigin::Acurast(_) => Ok(().into()), // nothing to be done for Acurast
			MultiOrigin::Tezos(_) => {
				// TODO: reenable
				// AcurastHyperdriveOutgoingTezos::send_message(Action::FinalizeJob(
				// 	job_id_seq.clone(),
				// 	refund,
				// ))
				// .map_err(|_| DispatchError::Other("Could not send FINALIZE_JOB to tezos").into())

				Ok(().into())
			},
			MultiOrigin::Ethereum(_) => {
				// TODO: reenable
				// HyperdriveOutgoingEthereum::send_message(
				//     Action::FinalizeJob(job_id_seq.clone(), refund),
				// )
				// .map_err(|_| DispatchError::Other("Could not send FINALIZE_JOB to ethereum").into())

				Ok(().into())
			},
			MultiOrigin::AlephZero(_) => {
				AcurastHyperdrive::send_to_proxy(
					ProxyChain::AlephZero,
					IncomingAction::FinalizeJob(job_id_seq.clone(), refund),
					&HyperdriveIbcFeePalletAccount::get(),
				)?;

				Ok(().into())
			},
			MultiOrigin::Vara(_) => {
				AcurastHyperdrive::send_to_proxy(
					ProxyChain::Vara,
					IncomingAction::FinalizeJob(job_id_seq.clone(), refund),
					&HyperdriveIbcFeePalletAccount::get(),
				)?;

				Ok(().into())
			},
		}
	}
}
