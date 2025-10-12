#![allow(deprecated)]

use frame_support::{
	traits::{GetStorageVersion, IsType, StorageVersion},
	weights::Weight,
	BoundedVec,
};
use sp_core::{ConstU32, Get};

use super::*;

type MigrationFn = dyn Fn() -> (Weight, bool);

pub fn migrate<T: Config>() -> Weight
where
	<T as pallet_acurast::Config>::RegistrationExtra: IsType<
		RegistrationExtra<
			T::Balance,
			T::AccountId,
			T::MaxSlots,
			T::ProcessorVersion,
			T::MaxVersions,
		>,
	>,
{
	let migrations: [(u16, &MigrationFn); 1] = [(7, &migrate_to_v7::<T>)];

	let onchain_version = Pallet::<T>::on_chain_storage_version();
	let mut weight: Weight = Default::default();
	for (i, f) in migrations.into_iter() {
		let migrating_version = StorageVersion::new(i);
		if onchain_version < migrating_version {
			let (f_weight, completed) = f();
			weight += f_weight;
			if completed {
				migrating_version.put::<Pallet<T>>();
				weight = weight.saturating_add(T::DbWeight::get().writes(1));
			}
		}
	}

	weight
}

pub fn migrate_to_v7<T: Config>() -> (Weight, bool) {
	const CLEAR_LIMIT: u32 = 100;

	let mut migration_completed = false;
	let mut weight = T::DbWeight::get().reads(1);
	let cursor = V7MigrationState::<T>::get().map(|c| c.to_vec());
	if cursor.is_none() {
		crate::Pallet::<T>::deposit_event(Event::<T>::V7MigrationStarted);
	}
	let res = <StoredStorageCapacity<T>>::clear(CLEAR_LIMIT, cursor.as_deref());
	weight = weight.saturating_add(T::DbWeight::get().writes(res.backend as u64));

	if let Some(new_cursor) = res.maybe_cursor {
		let bounded_cursor: Option<BoundedVec<u8, ConstU32<80>>> = new_cursor.try_into().ok();
		V7MigrationState::<T>::set(bounded_cursor);
	} else {
		migration_completed = true;
		V7MigrationState::<T>::kill();
		crate::Pallet::<T>::deposit_event(Event::<T>::V7MigrationCompleted);
	}
	weight = weight.saturating_add(T::DbWeight::get().writes(1));

	(weight, migration_completed)
}
