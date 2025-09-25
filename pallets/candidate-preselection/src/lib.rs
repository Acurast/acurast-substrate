#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
mod traits;

pub use pallet::*;
pub use traits::*;

use frame_support::traits::ValidatorRegistration;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{
		pallet_prelude::*,
		traits::{EnsureOrigin, IsType, StorageVersion, ValidatorRegistration},
	};
	use frame_system::pallet_prelude::*;
	use sp_std::prelude::*;

	use crate::traits::WeightInfo;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type ValidatorId: Member
			+ Parameter
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ TryFrom<Self::AccountId>;
		type ValidatorRegistration: ValidatorRegistration<Self::ValidatorId>;
		type UpdateOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		type WeightInfo: WeightInfo;
	}

	pub(crate) const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Candidate Preselection list updated.
		CandidateAdded(T::ValidatorId),
		CandidateRemoved(T::ValidatorId),
	}

	#[pallet::storage]
	#[pallet::getter(fn job_id_sequence)]
	pub type CandidatePreselectionList<T: Config> =
		StorageMap<_, Blake2_128Concat, T::ValidatorId, ()>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(< T as Config >::WeightInfo::add_candidate())]
		pub fn add_candidate(
			origin: OriginFor<T>,
			candidate: T::ValidatorId,
		) -> DispatchResultWithPostInfo {
			T::UpdateOrigin::ensure_origin(origin)?;
			<CandidatePreselectionList<T>>::insert(&candidate, ());
			Self::deposit_event(Event::CandidateAdded(candidate));
			Ok(().into())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(< T as Config >::WeightInfo::remove_candidate())]
		pub fn remove_candidate(
			origin: OriginFor<T>,
			candidate: T::ValidatorId,
		) -> DispatchResultWithPostInfo {
			T::UpdateOrigin::ensure_origin(origin)?;
			<CandidatePreselectionList<T>>::remove(&candidate);
			Self::deposit_event(Event::CandidateRemoved(candidate));
			Ok(().into())
		}
	}
}

impl<T: Config> ValidatorRegistration<T::ValidatorId> for Pallet<T> {
	fn is_registered(id: &T::ValidatorId) -> bool {
		<CandidatePreselectionList<T>>::get(id).is_some()
			&& T::ValidatorRegistration::is_registered(id)
	}
}
