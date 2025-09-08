use frame_support::traits::{
	fungible::{Inspect, Mutate},
	nonfungibles::{Create, InspectEnumerable as NFTInspectEnumerable},
	tokens::{Fortitude, Precision, Preservation},
};
use sp_core::{ConstU128, ConstU32};
use sp_std::prelude::*;

use acurast_runtime_common::{types::Signature, weight};
use pallet_acurast::ElegibleRewardAccountLookup;

#[cfg(feature = "runtime-benchmarks")]
use crate::benchmarking;
use crate::{
	Acurast, AcurastCompute, AcurastMarketplace, AcurastProcessorManager, Balances,
	ManagerCollectionId, RootAccountId, Runtime, RuntimeEvent, RuntimeHoldReason, Uniques,
};

impl pallet_acurast_processor_manager::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Proof = Signature;
	type ManagerId = u128;
	type ManagerIdProvider = AcurastManagerIdProvider;
	type ComputeHooks = AcurastCompute;
	type ProcessorAssetRecovery = AcurastProcessorRecovery;
	type MaxPairingUpdates = ConstU32<20>;
	type MaxProcessorsInSetUpdateInfo = ConstU32<100>;
	type Counter = u64;
	type PairingProofExpirationTime = ConstU128<14_400_000>; // 4 hours
	type UnixTime = pallet_timestamp::Pallet<Runtime>;
	type Advertisement = pallet_acurast_marketplace::AdvertisementFor<Self>;
	type AdvertisementHandler = AdvertisementHandlerImpl;
	type Currency = Balances;
	type RuntimeHoldReason = RuntimeHoldReason;
	type EligibleRewardAccountLookup = ElegibleRewardAccountLookup<
		Self::AccountId,
		Acurast,
		AcurastProcessorManager,
		AcurastProcessorManager,
	>;
	type AttestationHandler = Acurast;
	type WeightInfo = weight::pallet_acurast_processor_manager::WeightInfo<Self>;
	type ExtensionWeightInfo =
		weight::pallet_acurast_processor_manager_benchmarking_extension::WeightInfo<Self>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarking::AcurastBenchmarkHelper;
}

pub struct AdvertisementHandlerImpl;
impl pallet_acurast_processor_manager::AdvertisementHandler<Runtime> for AdvertisementHandlerImpl {
	fn advertise_for(
		processor: &<Runtime as frame_system::Config>::AccountId,
		advertisement: &<Runtime as pallet_acurast_processor_manager::Config>::Advertisement,
	) -> sp_runtime::DispatchResult {
		AcurastMarketplace::do_advertise(processor, advertisement)
	}
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

pub struct AcurastProcessorRecovery;
impl pallet_acurast_processor_manager::ProcessorAssetRecovery<Runtime>
	for AcurastProcessorRecovery
{
	fn recover_assets(
		processor: &<Runtime as frame_system::Config>::AccountId,
		destination_account: &<Runtime as frame_system::Config>::AccountId,
	) -> frame_support::pallet_prelude::DispatchResult {
		let usable_balance = <Balances as Inspect<_>>::reducible_balance(
			processor,
			Preservation::Preserve,
			Fortitude::Polite,
		);
		if usable_balance > 0 {
			let burned = <Balances as Mutate<_>>::burn_from(
				processor,
				usable_balance,
				Preservation::Preserve,
				Precision::BestEffort,
				Fortitude::Polite,
			)?;
			Balances::mint_into(destination_account, burned)?;
		}

		Ok(())
	}
}
