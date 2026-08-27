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

use frame_support::{
	dispatch::DispatchClass,
	traits::{Get, ValidatorRegistration},
};
use pallet_session::SessionManager;
use sp_staking::SessionIndex;
use sp_std::{marker::PhantomData, vec::Vec};

const LOG_TARGET: &str = "runtime::candidate-preselection";

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{
		pallet_prelude::*,
		traits::{EnsureOrigin, StorageVersion, ValidatorRegistration},
	};
	use frame_system::pallet_prelude::*;
	use sp_std::prelude::*;

	use crate::traits::WeightInfo;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type ValidatorId: Member
			+ Parameter
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ TryFrom<Self::AccountId>;
		type ValidatorRegistration: ValidatorRegistration<Self::ValidatorId>;
		/// Validators that are exempt from preselection filtering.
		type ExemptValidators: Get<Vec<Self::ValidatorId>>;
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

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		/// Validators preselected at genesis.
		pub candidates: Vec<T::ValidatorId>,
	}

	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { candidates: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			for candidate in &self.candidates {
				<CandidatePreselectionList<T>>::insert(candidate, ());
			}
		}
	}

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

/// A [`SessionManager`] wrapper that filters accounts whose preselection was
/// removed out of the collator set produced by the inner session manager.
///
/// `pallet_collator_selection` only consults `ValidatorRegistration` when a
/// candidate registers or takes a slot, so without this filter a candidate
/// removed via `remove_candidate` would keep authoring blocks indefinitely.
pub struct PreselectionSessionManager<T, Inner>(PhantomData<(T, Inner)>);

impl<T: Config, Inner: SessionManager<T::ValidatorId>> SessionManager<T::ValidatorId>
	for PreselectionSessionManager<T, Inner>
{
	fn new_session(new_index: SessionIndex) -> Option<Vec<T::ValidatorId>> {
		let collators = Inner::new_session(new_index)?;
		let reads = collators.len().saturating_add(1) as u64;
		let exempt = T::ExemptValidators::get();

		let preselected: Vec<T::ValidatorId> = collators
			.into_iter()
			.filter(|id| exempt.contains(id) || <CandidatePreselectionList<T>>::contains_key(id))
			.collect();

		frame_system::Pallet::<T>::register_extra_weight_unchecked(
			<T as frame_system::Config>::DbWeight::get().reads(reads),
			DispatchClass::Mandatory,
		);

		if preselected.is_empty() {
			log::warn!(
				target: LOG_TARGET,
				"preselection filtered out every collator for session {new_index}, keeping the current validator set",
			);
			return None;
		}

		Some(preselected)
	}

	fn new_session_genesis(new_index: SessionIndex) -> Option<Vec<T::ValidatorId>> {
		Inner::new_session_genesis(new_index)
	}

	fn end_session(end_index: SessionIndex) {
		Inner::end_session(end_index)
	}

	fn start_session(start_index: SessionIndex) {
		Inner::start_session(start_index)
	}
}
