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
		traits::{Currency, ExistenceRequirement, Get},
		Blake2_128Concat,
	};
	use frame_system::pallet_prelude::{BlockNumberFor, *};
	use sp_runtime::traits::{Convert, IdentifyAccount, One, StaticLookup, Verify, Zero};
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
			let d = if let Some(d) = destination { T::Lookup::lookup(d)? } else { who.clone() };
			let c = if let Some(c) = claimer { T::Lookup::lookup(c)? } else { who };
			Self::do_vest(d, c)
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
				|v_| -> Result<(BalanceFor<T>, BalanceFor<T>), Error<T>> {
					let v = v_.as_mut().ok_or(Error::<T>::NotVesting)?;

					let to_transfer = v.vestable::<T::BlockNumberToBalance>(now);
					// NOTE: we know it should never go below zero but the checked_sub serves as a conservative check that we never withdraw more than remaining
					v.remaining =
						v.remaining.checked_sub(&to_transfer).ok_or(Error::<T>::InternalError)?;
					v.latest_vest = now;

					let remaining = v.remaining;

					if remaining.is_zero() {
						*v_ = None;
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
	}
}
