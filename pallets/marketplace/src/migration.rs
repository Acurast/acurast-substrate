#![allow(deprecated)]

use frame_support::{
    traits::{GetStorageVersion, StorageVersion},
    weights::Weight,
};
use pallet_acurast::JobModules;
use sp_core::Get;

use super::*;

pub mod v1 {
    use frame_support::pallet_prelude::*;
    use pallet_acurast::{MultiOrigin, ParameterBound};
    use sp_std::prelude::*;

    /// The resource advertisement by a source containing the base restrictions.
    #[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
    pub struct AdvertisementRestriction<AccountId, MaxAllowedConsumers: ParameterBound> {
        /// Maximum memory in bytes not to be exceeded during any job's execution.
        pub max_memory: u32,
        /// Maximum network requests per second not to be exceeded.
        pub network_request_quota: u8,
        /// Storage capacity in bytes not to be exceeded in matching. The associated fee is listed in [pricing].
        pub storage_capacity: u32,
        /// An optional array of the [AccountId]s of consumers whose jobs should get accepted. If the array is [None], then jobs from all consumers are accepted.
        pub allowed_consumers: Option<BoundedVec<MultiOrigin<AccountId>, MaxAllowedConsumers>>,
    }
}

pub fn migrate<T: Config>() -> Weight {
    let migrations: [(u16, &dyn Fn() -> Weight); 3] = [
        (2, &migrate_to_v2::<T>),
        (3, &migrate_to_v3::<T>),
        (4, &migrate_to_v4::<T>),
    ];

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

fn migrate_to_v2<T: Config>() -> Weight {
    StoredAdvertisementRestriction::<T>::translate_values::<
        v1::AdvertisementRestriction<T::AccountId, T::MaxAllowedConsumers>,
        _,
    >(|ad| {
        Some(AdvertisementRestriction {
            max_memory: ad.max_memory,
            network_request_quota: ad.network_request_quota,
            storage_capacity: ad.storage_capacity,
            allowed_consumers: ad.allowed_consumers,
            available_modules: JobModules::default(),
        })
    });
    let count = StoredAdvertisementRestriction::<T>::iter_values().count() as u64;
    T::DbWeight::get().reads_writes(count + 1, count + 1)
}

fn migrate_to_v3<T: Config>() -> Weight {
    let mut count = 0u32;
    // we know they are reasonably few items and we can clear them within a single migration
    count += StoredJobStatus::<T>::clear(10_000, None).loops;
    count += StoredAdvertisementRestriction::<T>::clear(10_000, None).loops;
    count += StoredAdvertisementPricing::<T>::clear(10_000, None).loops;
    count += StoredStorageCapacity::<T>::clear(10_000, None).loops;
    count += StoredReputation::<T>::clear(10_000, None).loops;
    count += StoredMatches::<T>::clear(10_000, None).loops;

    T::DbWeight::get().writes((count + 1).into())
}

fn migrate_to_v4<T: Config>() -> Weight {
    // clear again all storages since we want to clear at the same time as pallet acurast for consistent state
    migrate_to_v3::<T>()
}
