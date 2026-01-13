use frame_support::traits::OnRuntimeUpgrade;

use acurast_runtime_common::weight::RocksDbWeight;
use pallet_acurast_compute::InflationEnabled;

use crate::Runtime;

pub struct RuntimeUpgradeHandler;
impl OnRuntimeUpgrade for RuntimeUpgradeHandler {
	fn on_runtime_upgrade() -> sp_runtime::Weight {
		let mut weight = sp_runtime::Weight::zero();

		let enabled = InflationEnabled::<Runtime, ()>::get();
		weight = weight.saturating_add(RocksDbWeight::get().reads(1));

		if !enabled {
			InflationEnabled::<Runtime, ()>::set(true);
			weight = weight.saturating_add(RocksDbWeight::get().writes(1));
		}

		weight
	}
}
