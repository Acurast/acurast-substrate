#![allow(deprecated)]

use frame_support::{
	traits::{GetStorageVersion, StorageVersion},
	weights::Weight,
};
use pallet_acurast::{JobModules, JobRegistration, StoredJobRegistration};
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

pub mod v4 {
	use super::*;
	use frame_support::{pallet_prelude::*, Deserialize, Serialize};
	use parity_scale_codec::{Decode, Encode, MaxEncodedLen};

	/// A proposed [Match] becomes an [crate::Assignment] once it's acknowledged.
	#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
	pub struct Assignment<Reward> {
		/// The 0-based slot index assigned to the source.
		pub slot: u8,
		/// The start delay for the first execution and all the following executions.
		pub start_delay: u64,
		/// The fee owed to source for each execution.
		pub fee_per_execution: Reward,
		/// If this assignment was acknowledged.
		pub acknowledged: bool,
		/// Keeps track of the SLA.
		pub sla: SLA,
		/// Processor Pub Keys
		pub pub_keys: PubKeys,
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
		/// The number of execution slots to be assigned to distinct sources. Either all or no slot get assigned by matching.
		pub slots: u8,
		/// Reward offered for each slot and scheduled execution of the job.
		pub reward: Reward,
		/// Minimum reputation required to process job, in parts per million, `r ∈ [0, 1_000_000]`.
		pub min_reputation: Option<u128>,
		/// Optional match provided with the job requirements. If provided, it gets processed instantaneously during
		/// registration call and validation errors lead to abortion of the call.
		pub instant_match: Option<PlannedExecutions<AccountId, MaxSlots>>,
	}

	impl<Reward, AccountId, MaxSlots: ParameterBound>
		Into<crate::JobRequirements<Reward, AccountId, MaxSlots>>
		for JobRequirements<Reward, AccountId, MaxSlots>
	{
		fn into(self) -> crate::JobRequirements<Reward, AccountId, MaxSlots> {
			crate::JobRequirements {
				assignment_strategy: AssignmentStrategy::Single(self.instant_match),
				slots: self.slots,
				reward: self.reward,
				min_reputation: self.min_reputation,
			}
		}
	}
}

pub fn migrate<T: Config>() -> Weight {
	let migrations: [(u16, &dyn Fn() -> Weight); 4] = [
		(2, &migrate_to_v2::<T>),
		(3, &migrate_to_v3::<T>),
		(4, &migrate_to_v4::<T>),
		(5, &migrate_to_v5::<T>),
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

fn migrate_to_v5<T: Config>() -> Weight {
	let mut count: u64 = 0;

	StoredMatches::<T>::translate_values::<v4::Assignment<<T as Config>::Balance>, _>(|m| {
		Some(Assignment {
			slot: m.slot,
			start_delay: m.start_delay,
			fee_per_execution: m.fee_per_execution,
			acknowledged: m.acknowledged,
			sla: m.sla,
			pub_keys: m.pub_keys,
			execution: ExecutionSpecifier::All,
		})
	});
	count += StoredMatches::<T>::iter_values().count() as u64;

	StoredJobRegistration::<T>::translate_values::<
		JobRegistration<
			<T as frame_system::Config>::AccountId,
			<T as pallet_acurast::Config>::MaxAllowedSources,
			v4::JobRequirements<
				<T as Config>::Balance,
				<T as frame_system::Config>::AccountId,
				<T as pallet_acurast::Config>::MaxSlots,
			>,
		>,
		_,
	>(|m| {
		let req: JobRequirements<
			<T as Config>::Balance,
			<T as frame_system::Config>::AccountId,
			<T as pallet_acurast::Config>::MaxSlots,
		> = m.extra.into();
		let extra: <T as Config>::RegistrationExtra = req.into();
		Some(JobRegistration {
			script: m.script,
			allowed_sources: m.allowed_sources,
			allow_only_verified_sources: m.allow_only_verified_sources,
			schedule: m.schedule,
			memory: m.memory,
			network_requests: m.network_requests,
			storage: m.storage,
			required_modules: m.required_modules,
			extra: extra.into(),
		})
	});
	count += StoredJobRegistration::<T>::iter_values().count() as u64;

	T::DbWeight::get().reads_writes(count + 1, count + 1)
}
