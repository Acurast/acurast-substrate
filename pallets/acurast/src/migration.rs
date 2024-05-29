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
		/// The script to execute. It is a vector of bytes representing an utf8 string. The string needs to be an ipfs url that points to the script.
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
	let migrations: [(u16, &dyn Fn() -> Weight); 1] = [(5, &migrate_to_v5::<T>)];

	let on_chain_version = Pallet::<T>::on_chain_storage_version();
	let mut weight: Weight = Default::default();
	for (i, f) in migrations.into_iter() {
		if on_chain_version < StorageVersion::new(i) {
			weight += f();
		}
	}

	STORAGE_VERSION.put::<Pallet<T>>();
	weight + T::DbWeight::get().writes(1)
}

fn migrate_to_v5<T: Config>() -> Weight {
	let mut count: u64 = 0;

	count += ExecutionEnvironment::<T>::iter_values().count() as u64;
	ExecutionEnvironment::<T>::translate::<EnvironmentFor<T>, _>(|job_id, _, env| {
		match <StoredJobRegistration<T>>::get(&job_id.0, &job_id.1) {
			// Remove since this is a dead assignment
			None => None,
			Some(_) => Some(env),
		}
	});

	T::DbWeight::get().reads_writes(count + 1, count + 1)
}
