use core::marker::PhantomData;

use pallet_acurast::{ManagerLookup, PoolId, ProcessorInfoProvider, Version};
use sp_runtime::FixedU128;

/// Provides processor heartbeat, version, metrics and pairing info to the marketplace.
pub struct AcurastProcessorInfoProvider<Runtime>(PhantomData<Runtime>);
impl<Runtime> ProcessorInfoProvider for AcurastProcessorInfoProvider<Runtime>
where
	Runtime: pallet_acurast::Config<ProcessorVersion = Version>
		+ pallet_acurast_marketplace::Config
		+ pallet_acurast_processor_manager::Config
		+ pallet_acurast_compute::Config,
{
	type AccountId = Runtime::AccountId;
	type ProcessorVersion = Runtime::ProcessorVersion;

	fn last_seen(processor: &Self::AccountId) -> Option<u128> {
		pallet_acurast_processor_manager::Pallet::<Runtime>::processor_last_seen(processor)
	}

	fn processor_version(processor: &Self::AccountId) -> Option<Self::ProcessorVersion> {
		pallet_acurast_processor_manager::Pallet::<Runtime>::processor_version(processor)
	}

	fn last_processor_metric(processor: &Self::AccountId, pool_id: PoolId) -> Option<FixedU128> {
		Some(pallet_acurast_compute::Pallet::<Runtime>::metrics(processor, pool_id)?.metric)
	}

	fn has_manager(processor: &Self::AccountId) -> bool {
		pallet_acurast_processor_manager::Pallet::<Runtime>::lookup_manager_id(processor).is_some()
	}

	fn manager(processor: &Self::AccountId) -> Option<Self::AccountId> {
		pallet_acurast_processor_manager::Pallet::<Runtime>::lookup(processor)
			.map(|(manager, _)| manager)
	}
}
