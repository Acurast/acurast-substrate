use frame_support::{
    traits::{GetStorageVersion, StorageVersion},
    weights::Weight,
};
use sp_core::Get;

use super::*;

pub mod v1 {
    use acurast_common::{AllowedSources, Schedule, Script};
    use frame_support::pallet_prelude::*;
    use sp_std::prelude::*;

    #[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq)]
    pub struct JobRegistration<AccountId, MaxAllowedSources: Get<u32>, Extra> {
        /// The script to execute. It is a vector of bytes representing a utf8 string. The string needs to be a ipfs url that points to the script.
        pub script: Script,
        /// An optional array of the [AccountId]s allowed to fulfill the job. If the array is [None], then all sources are allowed.
        pub allowed_sources: Option<AllowedSources<AccountId, MaxAllowedSources>>,
        /// A boolean indicating if only verified sources can fulfill the job. A verified source is one that has provided a valid key attestation.
        pub allow_only_verified_sources: bool,
        /// The schedule describing the desired (multiple) execution(s) of the script.
        pub schedule: Schedule,
        /// Maximum memory bytes used during a single execution of the job.
        pub memory: u32,
        /// Maximum network request used during a single execution of the job.
        pub network_requests: u32,
        /// Maximum storage bytes used during the whole period of the job's executions.
        pub storage: u32,
        /// Extra parameters. This type can be configured through [Config::RegistrationExtra].
        pub extra: Extra,
    }
}

pub fn migrate<T: Config>() -> Weight {
    let migrations: [(u16, &dyn Fn() -> Weight); 2] =
        [(2, &migrate_to_v2::<T>), (3, &migrate_to_v3::<T>)];

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
    StoredJobRegistration::<T>::translate::<
        v1::JobRegistration<T::AccountId, T::MaxAllowedSources, T::RegistrationExtra>,
        _,
    >(|_k1, _k2, job| {
        Some(JobRegistration {
            script: job.script,
            allowed_sources: job.allowed_sources,
            allow_only_verified_sources: job.allow_only_verified_sources,
            schedule: job.schedule,
            memory: job.memory,
            network_requests: job.network_requests,
            storage: job.storage,
            required_modules: JobModules::default(),
            extra: job.extra,
        })
    });
    let count = StoredJobRegistration::<T>::iter().count() as u64;
    T::DbWeight::get().reads_writes(count + 1, count + 1)
}

fn migrate_to_v3<T: Config>() -> Weight {
    let mut count = 0u32;
    // we know they are reasonably few items and we can clear them within a single migration
    count += StoredJobRegistration::<T>::clear(10_000, None).loops;

    T::DbWeight::get().writes((count + 1).into())
}
