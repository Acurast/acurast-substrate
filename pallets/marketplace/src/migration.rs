#![allow(deprecated)]

use frame_support::{
	traits::{GetStorageVersion, IsType, StorageVersion},
	weights::Weight,
};
use pallet_acurast::{JobRegistration, StoredJobRegistration};
use sp_core::Get;

use super::*;

pub mod v6 {
	use super::*;
	use frame_support::{pallet_prelude::*, Deserialize, Serialize};
	use parity_scale_codec::{Decode, Encode, MaxEncodedLen};

	#[derive(
		RuntimeDebug,
		Encode,
		Decode,
		MaxEncodedLen,
		TypeInfo,
		Clone,
		PartialEq,
		Eq,
		Serialize,
		Deserialize,
	)]
	pub struct RegistrationExtra<
		Reward,
		AccountId,
		MaxSlots: ParameterBound,
		Version,
		MaxVersions: ParameterBound,
	> {
		pub requirements: JobRequirements<Reward, AccountId, MaxSlots, Version, MaxVersions>,
	}

	impl<Reward, AccountId, MaxSlots: ParameterBound, Version, MaxVersions: ParameterBound>
		From<RegistrationExtra<Reward, AccountId, MaxSlots, Version, MaxVersions>>
		for crate::RegistrationExtra<Reward, AccountId, MaxSlots, Version, MaxVersions>
	{
		fn from(
			value: RegistrationExtra<Reward, AccountId, MaxSlots, Version, MaxVersions>,
		) -> Self {
			Self { requirements: value.requirements.into() }
		}
	}

	/// Structure representing a job registration.
	#[derive(
		RuntimeDebug,
		Encode,
		Decode,
		MaxEncodedLen,
		TypeInfo,
		Clone,
		Eq,
		PartialEq,
		Serialize,
		Deserialize,
	)]
	pub struct JobRequirements<
		Reward,
		AccountId,
		MaxSlots: ParameterBound,
		Version,
		MaxVersions: ParameterBound,
	> {
		pub assignment_strategy: AssignmentStrategy<AccountId, MaxSlots>,
		pub slots: u8,
		pub reward: Reward,
		pub min_reputation: Option<u128>,
		pub processor_version: Option<ProcessorVersionRequirements<Version, MaxVersions>>,
	}

	impl<Reward, AccountId, MaxSlots: ParameterBound, Version, MaxVersions: ParameterBound>
		From<JobRequirements<Reward, AccountId, MaxSlots, Version, MaxVersions>>
		for crate::JobRequirements<Reward, AccountId, MaxSlots, Version, MaxVersions>
	{
		fn from(val: JobRequirements<Reward, AccountId, MaxSlots, Version, MaxVersions>) -> Self {
			Self {
				assignment_strategy: val.assignment_strategy,
				slots: val.slots,
				reward: val.reward,
				min_reputation: val.min_reputation,
				processor_version: val.processor_version,
				runtime: Runtime::NodeJS,
			}
		}
	}
}

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
	let migrations: [(u16, &dyn Fn() -> Weight); 1] = [(6, &migrate_to_v6::<T>)];

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

pub fn migrate_to_v6<T: Config>() -> Weight
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
	StoredJobRegistration::<T>::translate_values::<
		JobRegistration<
			T::AccountId,
			T::MaxAllowedSources,
			v6::RegistrationExtra<
				T::Balance,
				T::AccountId,
				T::MaxSlots,
				T::ProcessorVersion,
				T::MaxVersions,
			>,
		>,
		_,
	>(|old| {
		Some(JobRegistration {
			script: old.script,
			allowed_sources: old.allowed_sources,
			allow_only_verified_sources: old.allow_only_verified_sources,
			schedule: old.schedule,
			memory: old.memory,
			network_requests: old.network_requests,
			storage: old.storage,
			required_modules: old.required_modules,
			extra: RegistrationExtra { requirements: old.extra.requirements.into() }.into(),
		})
	});

	let count = StoredJobRegistration::<T>::iter_values().count() as u64;
	T::DbWeight::get().reads_writes(count + 1, count + 1)
}
