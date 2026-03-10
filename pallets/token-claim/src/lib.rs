#![cfg_attr(not(feature = "std"), no_std)]

extern crate core;

pub use pallet::*;
pub use traits::*;
pub use types::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
mod traits;

mod types;
pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
pub use benchmarking::BenchmarkHelper;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{
		dispatch::DispatchResult,
		pallet_prelude::*,
		traits::{Currency, EnsureOrigin, ExistenceRequirement, Get},
		Blake2_128Concat,
	};
	use frame_system::pallet_prelude::{BlockNumberFor, *};
	use sp_runtime::traits::{
		Convert, IdentifyAccount, One, Saturating, StaticLookup, Verify, Zero,
	};
	use sp_std::prelude::*;

	use super::*;

	type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;

	/// A pallet for token claiming and vesting.
	///
	/// This is V2 with integrated vesting. Funds are held by the pallet account and released over time.
	/// V1 relayed on external `VestedTransferer`.
	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	/// Configures the pallet.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The currency trait.
		type Currency: Currency<Self::AccountId>;

		/// Signature type for claim proofs.
		type Signature: Parameter + Member + Verify + MaxEncodedLen;

		/// Account that signs the claim proofs.
		type Signer: Get<Self::AccountId>;

		/// Account that funds the claims.
		type Funder: Get<Self::AccountId>;

		/// Duration for vesting schedules created by claims.
		type VestingDuration: Get<BlockNumberFor<Self>>;

		/// Convert the block number into a balance.
		type BlockNumberToBalance: Convert<BlockNumberFor<Self>, BalanceFor<Self>>;

		/// The type used to identify claim types.
		type ClaimTypeId: Parameter + Member + MaxEncodedLen + Default + Copy + One + Saturating;

		/// Origin that can manage claim types.
		type UpdateOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		type WeightInfo: WeightInfo;
		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper: BenchmarkHelper<Self>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A claim was processed.
		Claimed { account: T::AccountId, amount: BalanceFor<T> },
		/// A claim was processed and the amount vested. Replaces V1's `Claimed` event no longer emitted.
		ClaimedV2 { claimer: T::AccountId, destination: T::AccountId, amount: BalanceFor<T> },
		/// An amount has been vested. This indicates a change in funds available on destination account.
		/// The balance given is the amount which is left unvested (and thus still held by [`<T as Config>::Funder`]).
		Vested { claimer: T::AccountId, destination: T::AccountId, remaining: BalanceFor<T> },
		/// A multi-claim was processed.
		MultiClaimed {
			claim_type_id: T::ClaimTypeId,
			claimer: T::AccountId,
			destination: T::AccountId,
			amount: BalanceFor<T>,
		},
		/// An amount has been vested from a multi-claim.
		MultiVested {
			claim_type_id: T::ClaimTypeId,
			claimer: T::AccountId,
			destination: T::AccountId,
			remaining: BalanceFor<T>,
		},
		/// A new claim type was created.
		ClaimTypeCreated { claim_type_id: T::ClaimTypeId, config: ClaimTypeConfigFor<T> },
		/// A claim type was updated.
		ClaimTypeUpdated { claim_type_id: T::ClaimTypeId, config: ClaimTypeConfigFor<T> },
		/// A claim type was removed.
		ClaimTypeRemoved { claim_type_id: T::ClaimTypeId },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The claim has already been processed.
		AlreadyClaimed,
		/// The claim proof is invalid.
		InvalidClaim,
		/// The account given is not vesting.
		NotVesting,
		/// Amount to vest is too low to be transferred.
		VestAmountTooLow,
		/// A vesting schedule already exists for this destination and claimer combination.
		VestingAlreadyExists,
		/// An internal error that should never happen.
		InternalError,
		/// The claim type was not found.
		ClaimTypeNotFound,
	}

	/// Processed claims storage as a map `claim_account` -> [`ProcessedClaimFor<T>`].
	///
	/// This storage is used for V1 vesting (via vesting pallet) and V2 vesting (via vesting schedules in [`Vesting`]).
	///
	/// Vesters can differentiate the two cases by calling either of this from destination account:
	///
	/// ```text
	/// if VESTING_ID lock identifier on destination_account {
	///   vesting::vest()
	/// } else {
	///   acurastTokenClaim::vest()
	/// }
	/// ```
	#[pallet::storage]
	#[pallet::getter(fn claimed)]
	pub type Claimed<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, ProcessedClaimFor<T>>;

	/// Information regarding the vesting as a double map `(destination_account, claimer_account) -> vesting schedule`.
	///
	/// Only used for V2 vesting via vesting schedules in this pallet.
	///
	/// In contrast to vesting pallet, it
	///
	/// - keeps `VestingInfoFor::remaining` funds on the [`<T as Config>::Funder`] account
	/// - supports multiple schedules per account (identified by claimer)
	#[pallet::storage]
	#[pallet::getter(fn vesting)]
	pub type Vesting<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::AccountId,
		VestingInfoFor<T>,
	>;

	/// Next claim type ID to be assigned.
	#[pallet::storage]
	#[pallet::getter(fn next_claim_type_id)]
	pub type NextClaimTypeId<T: Config> = StorageValue<_, T::ClaimTypeId, ValueQuery>;

	/// Claim type configurations.
	#[pallet::storage]
	#[pallet::getter(fn claim_type_configs)]
	pub type ClaimTypeConfigs<T: Config> =
		StorageMap<_, Identity, T::ClaimTypeId, ClaimTypeConfigFor<T>>;

	/// Processed multi-claims as a double map `(claim_type_id, claim_account)` -> [`ProcessedClaimFor<T>`].
	#[pallet::storage]
	#[pallet::getter(fn multi_claimed)]
	pub type MultiClaimed<T: Config> = StorageDoubleMap<
		_,
		Identity,
		T::ClaimTypeId,
		Blake2_128Concat,
		T::AccountId,
		ProcessedClaimFor<T>,
	>;

	/// Vesting info for multi-claims as an N-map `(claim_type_id, destination, claimer)` -> vesting schedule.
	#[pallet::storage]
	#[pallet::getter(fn multi_vesting)]
	pub type MultiVesting<T: Config> = StorageNMap<
		_,
		(
			NMapKey<Identity, T::ClaimTypeId>,
			NMapKey<Blake2_128Concat, T::AccountId>,
			NMapKey<Blake2_128Concat, T::AccountId>,
		),
		VestingInfoFor<T>,
	>;

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		T::AccountId: IsType<<<T::Signature as Verify>::Signer as IdentifyAccount>::AccountId>,
	{
		/// Process a token claim with vesting.
		/// Funds are kept on [`<T as Config>::Funder`] and released over time.
		///
		/// The vesting schedule is identified by the combination of destination and claimer (who).
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::claim())]
		pub fn claim(
			origin: OriginFor<T>,
			proof: ClaimProofFor<T>,
			destination: AccountIdLookupOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			if Self::claimed(&who).is_some() {
				return Err(Error::<T>::AlreadyClaimed)?;
			}

			if !proof.validate(&who, T::Signer::get()) {
				#[cfg(not(feature = "runtime-benchmarks"))]
				return Err(Error::<T>::InvalidClaim)?;
			}

			let amount = proof.amount;
			let destination = <T::Lookup as StaticLookup>::lookup(destination)?;

			let length_as_balance = T::BlockNumberToBalance::convert(T::VestingDuration::get());
			let per_block = amount / length_as_balance.max(One::one());
			let current_block = <frame_system::Pallet<T>>::block_number();

			ensure!(
				Vesting::<T>::get(&destination, &who).is_none(),
				Error::<T>::VestingAlreadyExists
			);

			// Store the vesting schedule
			Vesting::<T>::insert(
				&destination,
				&who,
				VestingInfo {
					claimer: who.clone(),
					per_block,
					starting_block: current_block,
					latest_vest: current_block,
					remaining: amount,
				},
			);

			Claimed::<T>::insert(
				&who,
				ProcessedClaimFor::<T> { proof, destination: destination.clone() },
			);
			Self::deposit_event(Event::<T>::ClaimedV2 { claimer: who, destination, amount });

			Ok(())
		}

		/// Unlock any vested funds of the destination account.
		///
		/// If destination is not provided, uses origin as destination account.
		/// If claimer is not provided, uses origin as claimer account.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::vest())]
		pub fn vest(
			origin: OriginFor<T>,
			destination: Option<AccountIdLookupOf<T>>,
			claimer: Option<AccountIdLookupOf<T>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let dest =
				if let Some(dest) = destination { T::Lookup::lookup(dest)? } else { who.clone() };
			let claim = if let Some(claim) = claimer { T::Lookup::lookup(claim)? } else { who };
			Self::do_vest(dest, claim)
		}

		/// Create a new claim type configuration.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::create_claim_type())]
		pub fn create_claim_type(
			origin: OriginFor<T>,
			config: ClaimTypeConfigFor<T>,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;

			let id = NextClaimTypeId::<T>::get();
			NextClaimTypeId::<T>::put(id.saturating_add(T::ClaimTypeId::one()));
			ClaimTypeConfigs::<T>::insert(id, config.clone());

			Self::deposit_event(Event::<T>::ClaimTypeCreated { claim_type_id: id, config });
			Ok(())
		}

		/// Update an existing claim type configuration.
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::update_claim_type())]
		pub fn update_claim_type(
			origin: OriginFor<T>,
			id: T::ClaimTypeId,
			config: ClaimTypeConfigFor<T>,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;

			ensure!(ClaimTypeConfigs::<T>::contains_key(id), Error::<T>::ClaimTypeNotFound);
			ClaimTypeConfigs::<T>::insert(id, config.clone());

			Self::deposit_event(Event::<T>::ClaimTypeUpdated { claim_type_id: id, config });
			Ok(())
		}

		/// Remove a claim type configuration.
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::remove_claim_type())]
		pub fn remove_claim_type(origin: OriginFor<T>, id: T::ClaimTypeId) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;

			ensure!(ClaimTypeConfigs::<T>::contains_key(id), Error::<T>::ClaimTypeNotFound);
			ClaimTypeConfigs::<T>::remove(id);

			Self::deposit_event(Event::<T>::ClaimTypeRemoved { claim_type_id: id });
			Ok(())
		}

		/// Process a multi-claim with vesting using a configurable claim type.
		#[pallet::call_index(5)]
		#[pallet::weight(<T as Config>::WeightInfo::multi_claim())]
		pub fn multi_claim(
			origin: OriginFor<T>,
			claim_type_id: T::ClaimTypeId,
			proof: ClaimProofFor<T>,
			destination: AccountIdLookupOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let config =
				ClaimTypeConfigs::<T>::get(claim_type_id).ok_or(Error::<T>::ClaimTypeNotFound)?;

			if MultiClaimed::<T>::get(claim_type_id, &who).is_some() {
				return Err(Error::<T>::AlreadyClaimed)?;
			}

			if !proof.validate_with_claim_type(&who, config.signer, claim_type_id) {
				#[cfg(not(feature = "runtime-benchmarks"))]
				return Err(Error::<T>::InvalidClaim)?;
			}

			let amount = proof.amount;
			let destination = <T::Lookup as StaticLookup>::lookup(destination)?;

			let length_as_balance = T::BlockNumberToBalance::convert(config.vesting_duration);
			let per_block = amount / length_as_balance.max(One::one());
			let current_block = <frame_system::Pallet<T>>::block_number();

			ensure!(
				MultiVesting::<T>::get((claim_type_id, &destination, &who)).is_none(),
				Error::<T>::VestingAlreadyExists
			);

			MultiVesting::<T>::insert(
				(claim_type_id, &destination, &who),
				VestingInfo {
					claimer: who.clone(),
					per_block,
					starting_block: current_block,
					latest_vest: current_block,
					remaining: amount,
				},
			);

			MultiClaimed::<T>::insert(
				claim_type_id,
				&who,
				ProcessedClaimFor::<T> { proof, destination: destination.clone() },
			);
			Self::deposit_event(Event::<T>::MultiClaimed {
				claim_type_id,
				claimer: who,
				destination,
				amount,
			});

			Ok(())
		}

		/// Unlock vested funds from a multi-claim.
		#[pallet::call_index(6)]
		#[pallet::weight(<T as Config>::WeightInfo::multi_vest())]
		pub fn multi_vest(
			origin: OriginFor<T>,
			claim_type_id: T::ClaimTypeId,
			destination: Option<AccountIdLookupOf<T>>,
			claimer: Option<AccountIdLookupOf<T>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let dest =
				if let Some(dest) = destination { T::Lookup::lookup(dest)? } else { who.clone() };
			let claim = if let Some(claim) = claimer { T::Lookup::lookup(claim)? } else { who };
			Self::do_multi_vest(claim_type_id, dest, claim)
		}
	}

	impl<T: Config> Pallet<T> {
		/// Unlock any vested funds of `destination` for the given `claimer`.
		fn do_vest(destination: T::AccountId, claimer: T::AccountId) -> DispatchResult {
			let now = <frame_system::Pallet<T>>::block_number();

			// A single check and withdraw operation
			let (to_transfer, remaining) = Vesting::<T>::try_mutate(
				&destination,
				&claimer,
				|maybe_schedule| -> Result<(BalanceFor<T>, BalanceFor<T>), Error<T>> {
					let schedule = maybe_schedule.as_mut().ok_or(Error::<T>::NotVesting)?;

					let to_transfer = schedule.vestable::<T::BlockNumberToBalance>(now);
					// NOTE: we know it should never go below zero but the checked_sub serves as a conservative check that we never withdraw more than remaining
					schedule.remaining = schedule
						.remaining
						.checked_sub(&to_transfer)
						.ok_or(Error::<T>::InternalError)?;
					schedule.latest_vest = now;

					let remaining = schedule.remaining;

					if remaining.is_zero() {
						*maybe_schedule = None;
					}

					Ok((to_transfer, remaining))
				},
			)?;

			ensure!(to_transfer >= T::Currency::minimum_balance(), Error::<T>::VestAmountTooLow);

			// Transfer the vested portion from funder to destination
			T::Currency::transfer(
				&T::Funder::get(),
				&destination,
				to_transfer,
				ExistenceRequirement::AllowDeath,
			)?;

			Self::deposit_event(Event::<T>::Vested { claimer, destination, remaining });

			Ok(())
		}

		/// Unlock any vested funds from a multi-claim.
		fn do_multi_vest(
			claim_type_id: T::ClaimTypeId,
			destination: T::AccountId,
			claimer: T::AccountId,
		) -> DispatchResult {
			let config =
				ClaimTypeConfigs::<T>::get(claim_type_id).ok_or(Error::<T>::ClaimTypeNotFound)?;

			let now = <frame_system::Pallet<T>>::block_number();

			let (to_transfer, remaining) = MultiVesting::<T>::try_mutate(
				(claim_type_id, &destination, &claimer),
				|maybe_schedule| -> Result<(BalanceFor<T>, BalanceFor<T>), Error<T>> {
					let schedule = maybe_schedule.as_mut().ok_or(Error::<T>::NotVesting)?;

					let to_transfer = schedule.vestable::<T::BlockNumberToBalance>(now);
					schedule.remaining = schedule
						.remaining
						.checked_sub(&to_transfer)
						.ok_or(Error::<T>::InternalError)?;
					schedule.latest_vest = now;

					let remaining = schedule.remaining;

					if remaining.is_zero() {
						*maybe_schedule = None;
					}

					Ok((to_transfer, remaining))
				},
			)?;

			ensure!(to_transfer >= T::Currency::minimum_balance(), Error::<T>::VestAmountTooLow);

			T::Currency::transfer(
				&config.funder,
				&destination,
				to_transfer,
				ExistenceRequirement::AllowDeath,
			)?;

			Self::deposit_event(Event::<T>::MultiVested {
				claim_type_id,
				claimer,
				destination,
				remaining,
			});

			Ok(())
		}
	}
}
