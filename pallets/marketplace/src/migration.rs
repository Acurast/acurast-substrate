#![allow(deprecated)]

use frame_support::{
	traits::{GetStorageVersion, IsType, StorageVersion},
	weights::Weight,
};
use pallet_acurast::{JobRegistration, StoredJobRegistration};
use sp_core::Get;

use super::*;

pub mod v5 {
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
	pub struct RegistrationExtra<Reward, AccountId, MaxSlots: ParameterBound> {
		pub requirements: JobRequirements<Reward, AccountId, MaxSlots>,
	}

	impl<Reward, AccountId, MaxSlots: ParameterBound, Version, MaxVersions: ParameterBound>
		Into<crate::RegistrationExtra<Reward, AccountId, MaxSlots, Version, MaxVersions>>
		for RegistrationExtra<Reward, AccountId, MaxSlots>
	{
		fn into(
			self,
		) -> crate::RegistrationExtra<Reward, AccountId, MaxSlots, Version, MaxVersions> {
			crate::RegistrationExtra {
				requirements: crate::JobRequirements {
					assignment_strategy: self.requirements.assignment_strategy,
					slots: self.requirements.slots,
					reward: self.requirements.reward,
					min_reputation: self.requirements.min_reputation,
					processor_version: None,
				},
			}
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
	pub struct JobRequirements<Reward, AccountId, MaxSlots: ParameterBound> {
		/// The type of matching selected by the consumer.
		pub assignment_strategy: AssignmentStrategy<AccountId, MaxSlots>,
		/// The number of execution slots to be assigned to distinct sources. Either all or no slot get assigned by matching.
		pub slots: u8,
		/// Reward offered for each slot and scheduled execution of the job.
		pub reward: Reward,
		/// Minimum reputation required to process job, in parts per million, `r ∈ [0, 1_000_000]`.
		pub min_reputation: Option<u128>,
	}

	impl<Reward, AccountId, MaxSlots: ParameterBound, Version, MaxVersions: ParameterBound>
		From<JobRequirements<Reward, AccountId, MaxSlots>>
		for crate::JobRequirements<Reward, AccountId, MaxSlots, Version, MaxVersions>
	{
		fn from(val: JobRequirements<Reward, AccountId, MaxSlots>) -> Self {
			Self {
				assignment_strategy: val.assignment_strategy,
				slots: val.slots,
				reward: val.reward,
				min_reputation: val.min_reputation,
				processor_version: None,
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
	let migrations: [(u16, &dyn Fn() -> Weight); 1] = [(5, &migrate_to_v5::<T>)];

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

pub fn migrate_to_v5<T: Config>() -> Weight
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
			v5::RegistrationExtra<T::Balance, T::AccountId, T::MaxSlots>,
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
