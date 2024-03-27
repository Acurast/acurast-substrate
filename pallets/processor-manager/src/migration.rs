#![allow(deprecated)]

use frame_support::{
	traits::{GetStorageVersion, StorageVersion},
	weights::Weight,
};
use sp_core::Get;

use super::*;

pub fn migrate<T: Config>() -> Weight {
	let migrations: [(u16, &dyn Fn() -> Weight); 1] = [(1, &migrate_to_v1::<T>)];

	let onchain_version = Pallet::<T>::on_chain_storage_version();
	let mut weight: Weight = Default::default();
	for (i, f) in migrations.into_iter() {
		if onchain_version < StorageVersion::new(i) {
			weight += f();
		}
	}

	STORAGE_VERSION.put::<Pallet<T>>();
	weight + T::DbWeight::get().writes(1)
}

fn migrate_to_v1<T: Config>() -> Weight {
	ApiVersion::<T>::put(1);
	T::DbWeight::get().writes(1)
}
