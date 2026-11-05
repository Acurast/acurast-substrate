use frame_support::traits::nonfungibles::{Create, InspectEnumerable as NFTInspectEnumerable};
use sp_core::{ConstU128, ConstU32};
use sp_std::prelude::*;

use acurast_runtime_common::{types::Signature, weight};
use pallet_acurast::ManagerProviderForEligibleProcessor;

#[cfg(feature = "runtime-benchmarks")]
use crate::benchmarking;
use crate::{
	Acurast, AcurastCompute, AcurastMarketplace, AcurastProcessorManager, Balances,
	EnsureCouncilOrRoot, ManagerCollectionId, RootAccountId, Runtime, RuntimeHoldReason, Uniques,
};

impl pallet_acurast_processor_manager::Config for Runtime {
	type Proof = Signature;
	type ManagerId = u128;
	type ManagerIdProvider = AcurastManagerIdProvider;
	type ComputeHooks = AcurastCompute;
	type MaxPairingUpdates = ConstU32<20>;
	type MaxProcessorsInSetUpdateInfo = ConstU32<100>;
	type Counter = u64;
	type PairingProofExpirationTime = ConstU128<14_400_000>; // 4 hours
	type UnixTime = pallet_timestamp::Pallet<Runtime>;
	type Advertisement = pallet_acurast_marketplace::AdvertisementFor<Self>;
	type MaxAllowedConsumers = <Runtime as pallet_acurast_marketplace::Config>::MaxAllowedConsumers;
	type AdvertisementHandler = AcurastMarketplace;
	type Currency = Balances;
	type RuntimeHoldReason = RuntimeHoldReason;
	type ManagerProviderForEligibleProcessor = ManagerProviderForEligibleProcessor<
		Self::AccountId,
		Self::ManagerId,
		Acurast,
		AcurastProcessorManager,
		AcurastProcessorManager,
	>;
	type AttestationHandler = Acurast;
	type OnProcessorUnpaired = (Acurast, AcurastCompute, AcurastMarketplace);
	type UpdateOrigin = EnsureCouncilOrRoot;
	type WeightInfo = weight::pallet_acurast_processor_manager::WeightInfo<Self>;
	type ExtensionWeightInfo =
		weight::pallet_acurast_processor_manager_onboarding_extension::WeightInfo<Self>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarking::AcurastBenchmarkHelper;
}

pub struct AcurastManagerIdProvider;
impl
	pallet_acurast::ManagerIdProvider<
		<Runtime as frame_system::Config>::AccountId,
		<Runtime as pallet_acurast_processor_manager::Config>::ManagerId,
	> for AcurastManagerIdProvider
{
	fn create_manager_id(
		id: <Runtime as pallet_acurast_processor_manager::Config>::ManagerId,
		owner: &<Runtime as frame_system::Config>::AccountId,
	) -> frame_support::pallet_prelude::DispatchResult {
		if Uniques::collection_owner(ManagerCollectionId::get()).is_none() {
			Uniques::create_collection(
				&ManagerCollectionId::get(),
				&RootAccountId::get(),
				&RootAccountId::get(),
			)?;
		}
		Uniques::do_mint(ManagerCollectionId::get(), id, owner.clone(), |_| Ok(()))
	}

	fn manager_id_for(
		owner: &<Runtime as frame_system::Config>::AccountId,
	) -> Result<
		<Runtime as pallet_acurast_processor_manager::Config>::ManagerId,
		sp_runtime::DispatchError,
	> {
		Uniques::owned_in_collection(&ManagerCollectionId::get(), owner)
			.nth(0)
			.ok_or(frame_support::pallet_prelude::DispatchError::Other("Manager ID not found"))
	}

	fn owner_for(
		manager_id: <Runtime as pallet_acurast_processor_manager::Config>::ManagerId,
	) -> Result<
		<Runtime as frame_system::Config>::AccountId,
		frame_support::pallet_prelude::DispatchError,
	> {
		Uniques::owner(ManagerCollectionId::get(), manager_id).ok_or(
			frame_support::pallet_prelude::DispatchError::Other(
				"Onwer for provided Manager ID not found",
			),
		)
	}
}
