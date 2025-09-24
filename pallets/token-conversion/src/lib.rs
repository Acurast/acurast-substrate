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
			fungible::{Inspect, InspectFreeze, Mutate, MutateFreeze},
			tokens::{Fortitude, Precision, Preservation},
			EnsureOrigin, Get,
		},
		Blake2_128Concat, PalletId,
	};
	use frame_system::pallet_prelude::{BlockNumberFor, *};
	use pallet_acurast::ProxyChain;
	use pallet_acurast_hyperdrive_ibc::{
		Layer, MessageBody, MessageId, MessageProcessor, MessageSender, Subject, SubjectFor,
	};
	use parity_scale_codec::Encode;
	use sp_runtime::{
		traits::{AccountIdConversion, Hash, Saturating, Zero},
		Perquintill, SaturatedConversion,
	};
	use sp_std::{prelude::*, vec};

	use super::*;

	/// A instantiable pallet for receiving secure state synchronizations into Acurast.
	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	/// Configures the pallet.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type PalletId: Get<PalletId>;
		type Chain: Get<ProxyChain>;
		type SendTo: Get<Option<SubjectFor<Self>>>;
		type ReceiveFrom: Get<Option<SubjectFor<Self>>>;
		type Currency: Inspect<Self::AccountId>
			+ InspectFreeze<Self::AccountId, Id = Self::RuntimeFreezeReason>
			+ MutateFreeze<Self::AccountId, Id = Self::RuntimeFreezeReason>
			+ Mutate<Self::AccountId>;
		type RuntimeFreezeReason: From<FreezeReason>;
		type Liquidity: Get<BalanceFor<Self>>;
		type MinLockDuration: Get<BlockNumberFor<Self>>;
		type MaxLockDuration: Get<BlockNumberFor<Self>>;
		type MessageSender: MessageSender<Self, BalanceFor<Self>>;
		type MessageIdHasher: Hash<Output = MessageId> + TypeInfo;
		#[pallet::constant]
		type ConvertTTL: Get<BlockNumberFor<Self>>;
		type Hook: TokenConversionHook;
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
		InvalidSubject,
		DecodingFailure,
		ConversionLockNotFound,
		InvalidDuration,
		ConversionLockUpdateDeadlinePassed,
		ConversionLockAlreadyUpdated,
		CannotUnlock,
		InitiatedConversionNotFound,
		NotEnabled,
	}

	#[pallet::storage]
	#[pallet::getter(fn initiated_conversion)]
	pub type InitiatedConversion<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, BalanceFor<T>>;

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

	/// A reason for placing a hold on funds.
	#[pallet::composite_enum]
	pub enum FreezeReason {
		/// Funds converted from canary.
		#[codec(index = 0)]
		Conversion,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		BalanceFor<T>: IsType<u128> + From<u64>,
	{
		#[pallet::call_index(0)]
		#[pallet::weight(< T as Config>::WeightInfo::convert())]
		pub fn convert(origin: OriginFor<T>, fee: BalanceFor<T>) -> DispatchResult {
			Self::ensure_enabled()?;
			let who = ensure_signed(origin)?;
			let Some(destination) = T::SendTo::get() else {
				return Err(Error::<T>::ConvertToNotEnabled)?;
			};
			if Self::initiated_conversion(&who).is_some() {
				return Err(Error::<T>::AlreadyConverted)?;
			}
			T::Hook::on_initiate_conversion()?;
			let reducible_balance =
				T::Currency::reducible_balance(&who, Preservation::Preserve, Fortitude::Polite);
			if reducible_balance < fee {
				return Err(Error::<T>::CannotPayFee)?;
			}
			if reducible_balance - fee < T::Liquidity::get() {
				return Err(Error::<T>::BalanceTooLow)?;
			}

			let burnable_balance = reducible_balance - fee - T::Liquidity::get();
			let burned = T::Currency::burn_from(
				&who,
				burnable_balance,
				Preservation::Preserve,
				Precision::Exact,
				Fortitude::Polite,
			)?;

			if burnable_balance != burned {
				return Err(Error::<T>::BalanceTooLow)?;
			}

			<InitiatedConversion<T>>::insert(&who, burned);
			Self::send_convert_message(&who, None, burned, fee, destination)?;
			Self::deposit_event(Event::<T>::ConversionInitiated { account: who, amount: burned });

			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(< T as Config>::WeightInfo::update_lock_duration())]
		pub fn update_lock_duration(
			origin: OriginFor<T>,
			new_duration: BlockNumberFor<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			<LockedConversion<T>>::mutate::<_, DispatchResult, _>(&who, |locked_conversion| {
				let Some(locked_conversion) = locked_conversion else {
					return Err(Error::<T>::ConversionLockNotFound)?;
				};
				if locked_conversion.modified {
					return Err(Error::<T>::ConversionLockAlreadyUpdated)?;
				}
				if new_duration < T::MinLockDuration::get()
					|| new_duration > T::MaxLockDuration::get()
				{
					return Err(Error::<T>::InvalidDuration)?;
				}
				let current_block_number = <frame_system::Pallet<T>>::block_number();
				let update_deadline = locked_conversion.lock_start + T::MinLockDuration::get();
				if current_block_number > update_deadline {
					return Err(Error::<T>::ConversionLockUpdateDeadlinePassed)?;
				}
				let factor = Perquintill::from_rational(
					new_duration.saturated_into::<u128>(),
					T::MaxLockDuration::get().saturated_into::<u128>(),
				);
				let new_amount = factor.mul_floor(locked_conversion.amount);
				let amount_diff = locked_conversion.amount.saturating_sub(new_amount);
				if !amount_diff.is_zero() {
					T::Currency::set_freeze(&FreezeReason::Conversion.into(), &who, new_amount)?;
					T::Currency::transfer(
						&who,
						&T::PalletId::get().into_account_truncating(),
						amount_diff,
						Preservation::Protect,
					)?;
				}
				locked_conversion.amount = new_amount;
				locked_conversion.lock_duration = new_duration;
				locked_conversion.modified = true;
				Ok(())
			})?;

			Self::deposit_event(Event::ConversionLockDurationUpdate { account: who });

			Ok(())
		}

		#[pallet::call_index(2)]
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
					let unlock_after =
						locked_conversion.lock_start + locked_conversion.lock_duration;
					if current_block_number < unlock_after {
						return Err(Error::<T>::CannotUnlock)?;
					}
					T::Currency::thaw(&FreezeReason::Conversion.into(), &who)?;
					*maybe_locked_conversion = None;
					Ok(())
				},
			)?;

			Self::deposit_event(Event::ConversionUnlocked { account: who });

			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(< T as Config>::WeightInfo::retry_convert())]
		pub fn retry_convert(origin: OriginFor<T>, fee: BalanceFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let Some(destination) = T::SendTo::get() else {
				return Err(Error::<T>::ConvertToNotEnabled)?;
			};
			let Some(burned) = Self::initiated_conversion(&who) else {
				return Err(Error::<T>::ConversionLockNotFound)?;
			};
			Self::send_convert_message(&who, None, burned, fee, destination)?;
			Self::deposit_event(Event::ConversionRetried { account: who });
			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(< T as Config>::WeightInfo::retry_convert_for())]
		pub fn retry_convert_for(
			origin: OriginFor<T>,
			account: T::AccountId,
			fee: BalanceFor<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let Some(destination) = T::SendTo::get() else {
				return Err(Error::<T>::ConvertToNotEnabled)?;
			};
			let Some(burned) = Self::initiated_conversion(&account) else {
				return Err(Error::<T>::ConversionLockNotFound)?;
			};
			Self::send_convert_message(&account, Some(&who), burned, fee, destination)?;
			Self::deposit_event(Event::ConversionRetried { account });
			Ok(())
		}

		#[pallet::call_index(5)]
		#[pallet::weight(< T as Config>::WeightInfo::retry_process_conversion())]
		pub fn retry_process_conversion(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			if T::ReceiveFrom::get().is_none() {
				return Err(Error::<T>::ConvertFromNotEnabled)?;
			};
			let Some(conversion_message) = <UnprocessedConversion<T>>::take(&who) else {
				return Err(Error::<T>::ConversionLockNotFound)?;
			};
			Self::process_conversion(conversion_message)
		}

		#[pallet::call_index(6)]
		#[pallet::weight(< T as Config>::WeightInfo::retry_process_conversion_for())]
		pub fn retry_process_conversion_for(
			origin: OriginFor<T>,
			account: T::AccountId,
		) -> DispatchResult {
			_ = ensure_signed(origin)?;
			if T::ReceiveFrom::get().is_none() {
				return Err(Error::<T>::ConvertFromNotEnabled)?;
			};
			let Some(conversion_message) = <UnprocessedConversion<T>>::take(&account) else {
				return Err(Error::<T>::ConversionLockNotFound)?;
			};
			Self::process_conversion(conversion_message)
		}

		#[pallet::call_index(7)]
		#[pallet::weight(< T as Config>::WeightInfo::retry_process_conversion_for())]
		pub fn set_enabled(origin: OriginFor<T>, enable: bool) -> DispatchResult {
			_ = T::EnableOrigin::ensure_origin(origin)?;
			<Enabled<T>>::set(enable);
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn ensure_enabled() -> Result<(), Error<T>> {
			if !Self::enabled() {
				return Err(Error::<T>::NotEnabled);
			}
			Ok(())
		}

		fn subject_for(proxy: ProxyChain) -> Result<SubjectFor<T>, Error<T>> {
			match proxy {
				ProxyChain::Acurast => Ok(Subject::Acurast(Layer::Extrinsic(
					T::PalletId::get().into_account_truncating(),
				))),
				ProxyChain::AcurastCanary => {
					Ok(Subject::AcurastCanary(T::PalletId::get().into_account_truncating()))
				},
				_ => Err(Error::<T>::InvalidSubject),
			}
		}

		fn send_convert_message(
			account: &T::AccountId,
			fee_account: Option<&T::AccountId>,
			amount: BalanceFor<T>,
			fee: BalanceFor<T>,
			destination: SubjectFor<T>,
		) -> DispatchResult {
			let pallet_account: T::AccountId = T::PalletId::get().into_account_truncating();
			if !fee.is_zero() {
				let transferred = T::Currency::transfer(
					fee_account.unwrap_or(account),
					&pallet_account,
					fee,
					Preservation::Preserve,
				)?;
				if transferred != fee {
					return Err(Error::<T>::CannotPayFee)?;
				}
			}
			let nonce = [pallet_account.encode().as_slice(), account.encode().as_slice()].concat();
			let message = ConversionMessageFor::<T> { account: account.clone(), amount };
			let payload = message.encode();
			T::MessageSender::send_message(
				Self::subject_for(T::Chain::get())?,
				&pallet_account,
				T::MessageIdHasher::hash(nonce.as_slice()),
				destination,
				payload,
				T::ConvertTTL::get(),
				fee,
			)
		}

		fn process_conversion(conversion_message: ConversionMessageFor<T>) -> DispatchResult {
			if Self::locked_conversion(&conversion_message.account).is_some() {
				// we just silently ignore multiple conversion messages for the same account
				return Ok(());
			}
			// if there is an unprocessed message for the same account, we process that one instead (first message wins).
			let conversion_message = <UnprocessedConversion<T>>::take(&conversion_message.account)
				.unwrap_or(conversion_message);
			let fund_result = Self::fund(&conversion_message);
			match fund_result {
				Ok(balance) => {
					let freeze_result = Self::freeze(balance, &conversion_message);
					match freeze_result {
						Ok(_) => {
							Self::deposit_event(Event::<T>::ConversionProcessed {
								account: conversion_message.account,
								amount: conversion_message.amount,
							});
						},
						Err(_) => {
							_ = Self::undo_fund(&conversion_message);
							<UnprocessedConversion<T>>::insert(
								&conversion_message.account,
								&conversion_message,
							);
							Self::deposit_event(Event::ConversionNotProcessed {
								account: conversion_message.account,
								amount: conversion_message.amount,
							});
						},
					}
				},
				Err(_) => {
					<UnprocessedConversion<T>>::insert(
						&conversion_message.account,
						&conversion_message,
					);
					Self::deposit_event(Event::ConversionNotProcessed {
						account: conversion_message.account,
						amount: conversion_message.amount,
					});
				},
			}
			Ok(())
		}

		fn fund(
			conversion_message: &ConversionMessageFor<T>,
		) -> Result<BalanceFor<T>, DispatchError> {
			let pallet_account = T::PalletId::get().into_account_truncating();
			T::Currency::transfer(
				&pallet_account,
				&conversion_message.account,
				conversion_message.amount,
				Preservation::Protect,
			)
		}

		fn undo_fund(
			conversion_message: &ConversionMessageFor<T>,
		) -> Result<BalanceFor<T>, DispatchError> {
			let pallet_account = T::PalletId::get().into_account_truncating();
			T::Currency::transfer(
				&conversion_message.account,
				&pallet_account,
				conversion_message.amount,
				Preservation::Expendable,
			)
		}

		fn freeze(
			transferred_balance: BalanceFor<T>,
			conversion_message: &ConversionMessageFor<T>,
		) -> DispatchResult {
			let mut freeze_amount = transferred_balance.saturating_sub(T::Liquidity::get());
			if freeze_amount.is_zero() {
				// in case there is not enough to leave Liquidity as free balance, we freeze everything
				freeze_amount = transferred_balance;
			}
			T::Currency::set_freeze(
				&FreezeReason::Conversion.into(),
				&conversion_message.account,
				freeze_amount,
			)?;
			let current_block_number = <frame_system::Pallet<T>>::block_number();
			let conversion = Conversion {
				amount: freeze_amount,
				lock_start: current_block_number,
				lock_duration: T::MaxLockDuration::get(),
				modified: false,
			};
			<LockedConversion<T>>::insert(&conversion_message.account, conversion);
			Ok(())
		}
	}

	impl<T: Config> MessageProcessor<T::AccountId, T::AccountId> for Pallet<T> {
		fn process(message: MessageBody<T::AccountId, T::AccountId>) -> DispatchResultWithPostInfo {
			Self::ensure_enabled()?;
			let Some(expected_sender) = T::ReceiveFrom::get() else {
				return Err(Error::<T>::ConvertFromNotEnabled)?;
			};
			if expected_sender != message.sender {
				return Err(Error::<T>::InvalidSender)?;
			}
			let decoded_message =
				ConversionMessageFor::<T>::decode(&mut message.payload.as_slice())
					.map_err(|_| Error::<T>::DecodingFailure)?;
			Self::process_conversion(decoded_message)?;
			Ok(().into())
		}
	}
}
