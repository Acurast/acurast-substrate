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

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{
		dispatch::DispatchResult,
		pallet_prelude::*,
		traits::{
			fungible::{Balanced, BalancedHold, Credit, Inspect, InspectHold, Mutate, MutateHold},
			tokens::{Fortitude, Precision, Preservation},
			EnsureOrigin, Get, OnUnbalanced,
		},
		Blake2_128Concat, PalletId,
	};
	use frame_system::pallet_prelude::{BlockNumberFor, *};
	use sp_runtime::{
		traits::{AccountIdConversion, Saturating, Zero},
		Perquintill, SaturatedConversion,
	};
	use sp_std::prelude::*;

	use acurast_common::Slashable;

	use super::*;

	/// A instantiable pallet for receiving secure state synchronizations into Acurast.
	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	/// Configures the pallet.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		type PalletId: Get<PalletId>;
		type Currency: Inspect<Self::AccountId>
			+ InspectHold<Self::AccountId, Reason = Self::RuntimeHoldReason>
			+ MutateHold<Self::AccountId, Reason = Self::RuntimeHoldReason>
			+ Mutate<Self::AccountId>
			+ BalancedHold<Self::AccountId>;
		type RuntimeHoldReason: From<HoldReason>;
		type MinLockDuration: Get<BlockNumberFor<Self>>;
		type MaxLockDuration: Get<BlockNumberFor<Self>>;
		type OnSlash: OnUnbalanced<Credit<Self::AccountId, Self::Currency>>;
		type EnableOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		ConversionInitiated { account: T::AccountId, amount: BalanceFor<T> },
		ConversionProcessed { account: T::AccountId, amount: BalanceFor<T> },
		ConversionNotProcessed { account: T::AccountId, amount: BalanceFor<T> },
		ConversionLockDurationUpdate { account: T::AccountId },
		ConversionUnlocked { account: T::AccountId },
		ConversionRetried { account: T::AccountId },
	}

	#[pallet::error]
	pub enum Error<T> {
		CannotPayFee,
		BalanceTooLow,
		AlreadyConverted,
		ConvertToNotEnabled,
		ConvertFromNotEnabled,
		InvalidSender,
		DecodingFailure,
		ConversionLockNotFound,
		InvalidDuration,
		ConversionLockUpdateDeadlinePassed,
		ConversionLockAlreadyUpdated,
		CannotUnlock,
		InitiatedConversionNotFound,
		NotEnabled,
		LockedBalance,
		CannotUnlockYet,
		ConversionDenied,
	}

	#[pallet::storage]
	#[pallet::getter(fn initiated_conversion)]
	pub type InitiatedConversion<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		InitiatedConversionMessage<T::AccountId, BalanceFor<T>, BlockNumberFor<T>>,
	>;

	#[pallet::storage]
	#[pallet::getter(fn locked_conversion)]
	pub type LockedConversion<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, Conversion<BalanceFor<T>, BlockNumberFor<T>>>;

	#[pallet::storage]
	#[pallet::getter(fn unprocessed_conversion)]
	pub type UnprocessedConversion<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, ConversionMessageFor<T>>;

	#[pallet::storage]
	#[pallet::getter(fn enabled)]
	pub type Enabled<T: Config> = StorageValue<_, bool, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn denied_source)]
	pub type DeniedSource<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, bool>;

	/// A reason for placing a hold on funds.
	#[pallet::composite_enum]
	pub enum HoldReason {
		/// Funds migrated from canary.
		#[codec(index = 0)]
		Conversion,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		BalanceFor<T>: IsType<u128> + From<u64>,
	{
		/// DISABLED: cross-chain token conversion/migration has been removed.
		///
		/// The extrinsic is retained (with its original `call_index`) to keep the pallet's
		/// call metadata stable, but always fails.
		#[pallet::call_index(0)]
		#[pallet::weight(T::DbWeight::get().reads(1))]
		pub fn convert(origin: OriginFor<T>, _fee: BalanceFor<T>) -> DispatchResult {
			let _ = ensure_signed(origin)?;
			Err(Error::<T>::NotEnabled.into())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(< T as Config>::WeightInfo::unlock())]
		pub fn unlock(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			<LockedConversion<T>>::mutate::<_, DispatchResult, _>(
				&who,
				|maybe_locked_conversion| {
					let Some(locked_conversion) = maybe_locked_conversion else {
						return Ok(());
					};
					let current_block_number = <frame_system::Pallet<T>>::block_number();
					let lock_progress = current_block_number
						.saturating_sub(locked_conversion.lock_start)
						.min(T::MaxLockDuration::get());

					if lock_progress < T::MinLockDuration::get() {
						return Err(Error::<T>::CannotUnlockYet)?;
					}

					let amount_factor = Perquintill::from_rational(
						lock_progress.saturated_into::<u128>(),
						T::MaxLockDuration::get().saturated_into::<u128>(),
					);
					let amount_unlocked = amount_factor.mul_floor(locked_conversion.amount);
					let slash = locked_conversion.amount.saturating_sub(amount_unlocked);

					T::Currency::release_all(
						&HoldReason::Conversion.into(),
						&who,
						Precision::Exact,
					)?;

					if !slash.is_zero() {
						let reducible_balance = T::Currency::reducible_balance(
							&who,
							Preservation::Preserve,
							Fortitude::Polite,
						);
						if reducible_balance < slash {
							return Err(Error::<T>::CannotUnlock)?;
						}
						let imbalance = T::Currency::withdraw(
							&who,
							slash,
							Precision::Exact,
							Preservation::Preserve,
							Fortitude::Polite,
						)?;
						T::OnSlash::on_unbalanced(imbalance);
					}

					*maybe_locked_conversion = None;
					Ok(())
				},
			)?;

			Self::deposit_event(Event::ConversionUnlocked { account: who });

			Ok(())
		}

		/// DISABLED: cross-chain token conversion/migration has been removed. Retained for
		/// stable call metadata; always fails.
		#[pallet::call_index(2)]
		#[pallet::weight(T::DbWeight::get().reads(1))]
		pub fn retry_convert(origin: OriginFor<T>, _fee: BalanceFor<T>) -> DispatchResult {
			let _ = ensure_signed(origin)?;
			Err(Error::<T>::NotEnabled.into())
		}

		/// DISABLED: cross-chain token conversion/migration has been removed. Retained for
		/// stable call metadata; always fails.
		#[pallet::call_index(3)]
		#[pallet::weight(T::DbWeight::get().reads(1))]
		pub fn retry_convert_for(
			origin: OriginFor<T>,
			_account: T::AccountId,
			_fee: BalanceFor<T>,
		) -> DispatchResult {
			let _ = ensure_signed(origin)?;
			Err(Error::<T>::NotEnabled.into())
		}

		/// DISABLED: cross-chain token conversion/migration has been removed. Retained for
		/// stable call metadata; always fails.
		#[pallet::call_index(4)]
		#[pallet::weight(T::DbWeight::get().reads(1))]
		pub fn retry_process_conversion(origin: OriginFor<T>) -> DispatchResult {
			let _ = ensure_signed(origin)?;
			Err(Error::<T>::NotEnabled.into())
		}

		/// DISABLED: cross-chain token conversion/migration has been removed. Retained for
		/// stable call metadata; always fails.
		#[pallet::call_index(5)]
		#[pallet::weight(T::DbWeight::get().reads(1))]
		pub fn retry_process_conversion_for(
			origin: OriginFor<T>,
			_account: T::AccountId,
		) -> DispatchResult {
			let _ = ensure_signed(origin)?;
			Err(Error::<T>::NotEnabled.into())
		}

		#[pallet::call_index(6)]
		#[pallet::weight(< T as Config>::WeightInfo::set_enabled())]
		pub fn set_enabled(origin: OriginFor<T>, enable: bool) -> DispatchResult {
			_ = T::EnableOrigin::ensure_origin(origin)?;
			<Enabled<T>>::set(enable);
			Ok(())
		}

		#[pallet::call_index(7)]
		#[pallet::weight(< T as Config>::WeightInfo::deny_source())]
		pub fn deny_source(
			origin: OriginFor<T>,
			account: T::AccountId,
			denied: bool,
		) -> DispatchResult {
			_ = T::EnableOrigin::ensure_origin(origin)?;
			<DeniedSource<T>>::insert(&account, denied);
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn account_id() -> T::AccountId {
			T::PalletId::get().into_account_truncating()
		}
	}

	impl<T: Config> Slashable<T::AccountId> for Pallet<T> {
		type Currency = T::Currency;

		fn slash(account: &T::AccountId, amount: BalanceFor<T>) -> Option<ImbalanceFor<T>> {
			<LockedConversion<T>>::mutate(account, |conversion_| {
				let Some(conversion) = conversion_ else {
					return None;
				};
				let (credit, not_slashed) =
					T::Currency::slash(&HoldReason::Conversion.into(), account, amount);
				let new_amount =
					conversion.amount.saturating_sub(amount.saturating_sub(not_slashed));
				if !new_amount.is_zero() {
					conversion.amount = new_amount;
				} else {
					*conversion_ = None;
				}

				Some(credit)
			})
		}
	}
}
