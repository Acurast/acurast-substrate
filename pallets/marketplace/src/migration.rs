use frame_support::{
	traits::{GetStorageVersion, StorageVersion},
	weights::{Weight, WeightMeter},
	Blake2_128, BoundedVec,
};
use sp_core::Get;

use super::*;

/// Storage of the removed advertisement pricing, cleared by [`clear_pricing`].
#[frame_support::storage_alias]
pub(crate) type StoredAdvertisementPricing<T: Config> =
	StorageMap<Pallet<T>, Blake2_128, <T as frame_system::Config>::AccountId, PricingFor<T>>;

/// Storage version that removes [`StoredAdvertisementPricing`].
const TARGET: StorageVersion = StorageVersion::new(8);

/// Benchmarked proof size of one [`StoredAdvertisementPricing`] entry, trie overhead included.
const ENTRY_PROOF_SIZE: u64 = 2548;

/// Weight of removing one [`StoredAdvertisementPricing`] entry.
pub(crate) fn entry_weight<T: Config>() -> Weight {
	T::DbWeight::get()
		.writes(1)
		.saturating_add(Weight::from_parts(0, ENTRY_PROOF_SIZE))
}

/// Fixed weight of a cleanup step besides the cursor read: the cursor write.
pub(crate) fn step_weight<T: Config>() -> Weight {
	T::DbWeight::get().writes(1)
}

/// Sets [`TARGET`] and schedules the [`StoredAdvertisementPricing`] cleanup on chains below it.
pub fn on_runtime_upgrade<T: Config>() -> Weight {
	if Pallet::<T>::on_chain_storage_version() >= TARGET {
		return T::DbWeight::get().reads(1);
	}
	TARGET.put::<Pallet<T>>();
	MigrationCursor::<T>::put(BoundedVec::new());
	T::DbWeight::get().reads_writes(1, 2)
}

/// Clears as many [`StoredAdvertisementPricing`] entries as `meter` allows while a cleanup is
/// scheduled, resuming from [`MigrationCursor`] and removing it once the map is empty.
pub fn clear_pricing<T: Config>(meter: &mut WeightMeter) {
	if meter.try_consume(T::DbWeight::get().reads(1)).is_err() {
		return;
	}
	let Some(cursor) = MigrationCursor::<T>::get() else {
		return;
	};
	let Some(limit) = meter
		.remaining()
		.saturating_sub(step_weight::<T>())
		.checked_div_per_component(&entry_weight::<T>())
		.filter(|limit| *limit > 0)
	else {
		return;
	};
	meter.consume(step_weight::<T>());

	if cursor.is_empty() {
		Pallet::<T>::deposit_event(Event::<T>::V8MigrationStarted);
	}
	let res = StoredAdvertisementPricing::<T>::clear(
		limit.min(u32::MAX as u64) as u32,
		(!cursor.is_empty()).then_some(&cursor[..]),
	);
	meter.consume(entry_weight::<T>().saturating_mul(res.backend as u64));

	match res.maybe_cursor {
		// an unfitting cursor restarts the scan, which still progresses as cleared keys are gone
		Some(next) => MigrationCursor::<T>::put(BoundedVec::try_from(next).unwrap_or_default()),
		None => {
			MigrationCursor::<T>::kill();
			Pallet::<T>::deposit_event(Event::<T>::V8MigrationCompleted);
		},
	}
}
