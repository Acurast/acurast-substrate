use frame_support::{
    traits::{GetStorageVersion, StorageVersion},
    weights::Weight,
};
use sp_core::Get;

use super::*;

pub fn migrate<T: Config<I>, I: 'static>() -> Weight {
    let migrations: [(u16, &dyn Fn() -> Weight); 1] = [(2, &migrate_to_v2::<T, I>)];

    let onchain_version = Pallet::<T, I>::on_chain_storage_version();
    let mut weight: Weight = Default::default();
    for (i, f) in migrations.into_iter() {
        if onchain_version < StorageVersion::new(i) {
            weight += f();
        }
    }

    STORAGE_VERSION.put::<Pallet<T, I>>();
    weight + T::DbWeight::get().writes(1)
}

fn migrate_to_v2<T: Config<I>, I: 'static>() -> Weight {
    let mut count = 0u32;
    // we know they are reasonably few items and we can clear them within a single migration
    count += AssetIndex::<T, I>::clear(100, None).loops;
    count += ReverseAssetIndex::<T, I>::clear(100, None).loops;

    T::DbWeight::get().reads_writes((count + 1).into(), (count + 1).into())
}
