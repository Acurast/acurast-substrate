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
	use pallet_acurast::JobModules;
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
					min_cpu_score: None,
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
		Into<crate::JobRequirements<Reward, AccountId, MaxSlots, Version, MaxVersions>>
		for JobRequirements<Reward, AccountId, MaxSlots>
	{
		fn into(self) -> crate::JobRequirements<Reward, AccountId, MaxSlots, Version, MaxVersions> {
			crate::JobRequirements {
				assignment_strategy: self.assignment_strategy,
				slots: self.slots,
				reward: self.reward,
				min_reputation: self.min_reputation,
				processor_version: None,
				min_cpu_score: None,
			}
		}
	}

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
		/// The modules available to the job on processor.
		pub available_modules: JobModules,
	}

	impl<AccountId, MaxAllowedConsumers: ParameterBound>
		Into<crate::AdvertisementRestriction<AccountId, MaxAllowedConsumers>>
		for AdvertisementRestriction<AccountId, MaxAllowedConsumers>
	{
		fn into(self) -> crate::AdvertisementRestriction<AccountId, MaxAllowedConsumers> {
			crate::AdvertisementRestriction {
				max_memory: self.max_memory,
				network_request_quota: self.network_request_quota,
				storage_capacity: self.storage_capacity,
				allowed_consumers: self.allowed_consumers,
				available_modules: self.available_modules,
				cpu_score: 0,
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
	StoredAdvertisementRestriction::<T>::translate_values::<
		v5::AdvertisementRestriction<T::AccountId, T::MaxAllowedConsumers>,
		_,
	>(|old| Some(old.into()));

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

	let count = StoredAdvertisementRestriction::<T>::iter_values().count() as u64 +
		StoredJobRegistration::<T>::iter_values().count() as u64;
	T::DbWeight::get().reads_writes(count + 1, count + 1)
}
