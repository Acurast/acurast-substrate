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
			fungible::{Balanced, Credit, Inspect, InspectFreeze, Mutate, MutateFreeze},
			tokens::{Fortitude, Precision, Preservation},
			EnsureOrigin, Get, OnUnbalanced,
		},
		Blake2_128Concat, PalletId,
	};
	use frame_system::pallet_prelude::{BlockNumberFor, *};
	use parity_scale_codec::Encode;
	use sp_runtime::{
		traits::{AccountIdConversion, Hash, Saturating, Zero},
		Perquintill, SaturatedConversion,
	};
	use sp_std::{prelude::*, vec};

	use acurast_common::{
		Layer, MessageBody, MessageFeeProvider, MessageProcessor, MessageSender, ProxyChain,
		Subject,
	};

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
			+ Mutate<Self::AccountId>
			+ Balanced<Self::AccountId>;
		type RuntimeFreezeReason: From<FreezeReason>;
		type Liquidity: Get<BalanceFor<Self>>;
		type MaxLockDuration: Get<BlockNumberFor<Self>>;
		type MessageSender: MessageSender<
			Self::AccountId,
			Self::AccountId,
			BalanceFor<Self>,
			BlockNumberFor<Self>,
		>;
		type MessageIdHasher: Hash<
				Output = <Self::MessageSender as MessageSender<
					Self::AccountId,
					Self::AccountId,
					BalanceFor<Self>,
					BlockNumberFor<Self>,
				>>::MessageNonce,
			> + TypeInfo;
		type OnSlash: OnUnbalanced<Credit<Self::AccountId, Self::Currency>>;
		#[pallet::constant]
		type ConvertTTL: Get<BlockNumberFor<Self>>;
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
		LockedBalance,
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
				cfg_if::cfg_if! {
					if #[cfg(not(feature = "runtime-benchmarks"))] {
						return Err(Error::<T>::ConvertToNotEnabled)?;
					} else {
						return Ok(())
					}
				}
			};
			if Self::initiated_conversion(&who).is_some() {
				return Err(Error::<T>::AlreadyConverted)?;
			}
			let total_balance = T::Currency::balance(&who);
			let reducible_balance =
				T::Currency::reducible_balance(&who, Preservation::Preserve, Fortitude::Polite);
			let frozen_balance = total_balance
				.saturating_sub(reducible_balance.saturating_add(T::Currency::minimum_balance()));
			if !frozen_balance.is_zero() {
				return Err(Error::<T>::LockedBalance)?;
			}
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

			let current_block_number = <frame_system::Pallet<T>>::block_number();

			<InitiatedConversion<T>>::insert(
				&who,
				InitiatedConversionMessageFor::<T> {
					burned,
					fee_payer: who.clone(),
					started_at: current_block_number,
				},
			);
			Self::send_convert_message(&who, None, None, burned, fee, destination)?;
			Self::deposit_event(Event::<T>::ConversionInitiated { account: who, amount: burned });

			Ok(())
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
					let amount_factor = Perquintill::from_rational(
						lock_progress.saturated_into::<u128>(),
						T::MaxLockDuration::get().saturated_into::<u128>(),
					);
					let amount_unlocked = amount_factor.mul_floor(locked_conversion.amount);
					let slash = locked_conversion.amount.saturating_sub(amount_unlocked);

					T::Currency::thaw(&FreezeReason::Conversion.into(), &who)?;

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

		#[pallet::call_index(2)]
		#[pallet::weight(< T as Config>::WeightInfo::retry_convert())]
		pub fn retry_convert(origin: OriginFor<T>, fee: BalanceFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let Some(destination) = T::SendTo::get() else {
				cfg_if::cfg_if! {
					if #[cfg(not(feature = "runtime-benchmarks"))] {
						return Err(Error::<T>::ConvertToNotEnabled)?;
					} else {
						return Ok(())
					}
				}
			};
			let Some((burned, prev_payer)) = Self::update_initiated_conversion(&who, who.clone())
			else {
				return Err(Error::<T>::ConversionLockNotFound)?;
			};
			Self::send_convert_message(&who, None, Some(&prev_payer), burned, fee, destination)?;
			Self::deposit_event(Event::ConversionRetried { account: who });
			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(< T as Config>::WeightInfo::retry_convert_for())]
		pub fn retry_convert_for(
			origin: OriginFor<T>,
			account: T::AccountId,
			fee: BalanceFor<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let Some(destination) = T::SendTo::get() else {
				cfg_if::cfg_if! {
					if #[cfg(not(feature = "runtime-benchmarks"))] {
						return Err(Error::<T>::ConvertToNotEnabled)?;
					} else {
						return Ok(())
					}
				}
			};
			let Some((burned, prev_payer)) =
				Self::update_initiated_conversion(&account, who.clone())
			else {
				return Err(Error::<T>::ConversionLockNotFound)?;
			};
			Self::send_convert_message(
				&account,
				Some(&who),
				Some(&prev_payer),
				burned,
				fee,
				destination,
			)?;
			Self::deposit_event(Event::ConversionRetried { account });
			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(< T as Config>::WeightInfo::retry_process_conversion())]
		pub fn retry_process_conversion(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			if T::ReceiveFrom::get().is_none() {
				cfg_if::cfg_if! {
					if #[cfg(not(feature = "runtime-benchmarks"))] {
						return Err(Error::<T>::ConvertFromNotEnabled)?;
					} else {
						return Ok(())
					}
				}
			};
			let Some(conversion_message) = <UnprocessedConversion<T>>::take(&who) else {
				return Err(Error::<T>::ConversionLockNotFound)?;
			};
			Self::process_conversion(conversion_message)
		}

		#[pallet::call_index(5)]
		#[pallet::weight(< T as Config>::WeightInfo::retry_process_conversion_for())]
		pub fn retry_process_conversion_for(
			origin: OriginFor<T>,
			account: T::AccountId,
		) -> DispatchResult {
			_ = ensure_signed(origin)?;
			if T::ReceiveFrom::get().is_none() {
				cfg_if::cfg_if! {
					if #[cfg(not(feature = "runtime-benchmarks"))] {
						return Err(Error::<T>::ConvertFromNotEnabled)?;
					} else {
						return Ok(())
					}
				}
			};
			let Some(conversion_message) = <UnprocessedConversion<T>>::take(&account) else {
				return Err(Error::<T>::ConversionLockNotFound)?;
			};
			Self::process_conversion(conversion_message)
		}

		#[pallet::call_index(6)]
		#[pallet::weight(< T as Config>::WeightInfo::retry_process_conversion_for())]
		pub fn set_enabled(origin: OriginFor<T>, enable: bool) -> DispatchResult {
			_ = T::EnableOrigin::ensure_origin(origin)?;
			<Enabled<T>>::set(enable);
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn account_id() -> T::AccountId {
			T::PalletId::get().into_account_truncating()
		}

		fn ensure_enabled() -> Result<(), Error<T>> {
			if !Self::enabled() {
				return Err(Error::<T>::NotEnabled);
			}
			Ok(())
		}

		fn subject_for(proxy: ProxyChain) -> Result<SubjectFor<T>, Error<T>> {
			match proxy {
				ProxyChain::Acurast => Ok(Subject::Acurast(Layer::Extrinsic(Self::account_id()))),
				ProxyChain::AcurastCanary => {
					Ok(Subject::AcurastCanary(Layer::Extrinsic(Self::account_id())))
				},
				_ => Err(Error::<T>::InvalidSubject),
			}
		}

		fn update_initiated_conversion(
			account: &T::AccountId,
			new_fee_payer: T::AccountId,
		) -> Option<(BalanceFor<T>, T::AccountId)> {
			<InitiatedConversion<T>>::mutate::<_, Option<(BalanceFor<T>, T::AccountId)>, _>(
				account,
				|value| {
					let Some(initiated_conversion) = value else {
						return None;
					};
					let prev_payer =
						core::mem::replace(&mut initiated_conversion.fee_payer, new_fee_payer);
					Some((initiated_conversion.burned, prev_payer))
				},
			)
		}

		fn send_convert_message(
			account: &T::AccountId,
			fee_payer: Option<&T::AccountId>,
			prev_fee_payer: Option<&T::AccountId>,
			amount: BalanceFor<T>,
			fee: BalanceFor<T>,
			destination: SubjectFor<T>,
		) -> DispatchResult {
			let pallet_account = Self::account_id();
			let payer = fee_payer.unwrap_or(account);
			let prev_payer = prev_fee_payer.unwrap_or(account);
			if !fee.is_zero() {
				let transferred =
					T::Currency::transfer(payer, &pallet_account, fee, Preservation::Preserve)?;
				if transferred != fee {
					return Err(Error::<T>::CannotPayFee)?;
				}
			}
			let nonce = [pallet_account.encode().as_slice(), account.encode().as_slice()].concat();
			let message = ConversionMessageFor::<T> { account: account.clone(), amount };
			let payload = message.encode();
			let (_, maybe_replaced_message) = T::MessageSender::send_message(
				Self::subject_for(T::Chain::get())?,
				&pallet_account,
				T::MessageIdHasher::hash(nonce.as_slice()),
				destination,
				payload,
				T::ConvertTTL::get(),
				fee,
			)?;

			if let Some(replaced_message) = maybe_replaced_message {
				let fee = replaced_message.get_fee();
				if !fee.is_zero() {
					_ = T::Currency::transfer(
						&pallet_account,
						prev_payer,
						fee,
						Preservation::Preserve,
					)?;
				}
			}

			Ok(())
		}

		pub(crate) fn process_conversion(
			conversion_message: ConversionMessageFor<T>,
		) -> DispatchResult {
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
			let pallet_account = Self::account_id();
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
			let pallet_account = Self::account_id();
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
			let conversion = Conversion { amount: freeze_amount, lock_start: current_block_number };
			<LockedConversion<T>>::insert(&conversion_message.account, conversion);
			Ok(())
		}
	}

	impl<T: Config> MessageProcessor<T::AccountId, T::AccountId> for Pallet<T> {
		fn process(
			message: impl MessageBody<T::AccountId, T::AccountId>,
		) -> DispatchResultWithPostInfo {
			let Some(expected_sender) = T::ReceiveFrom::get() else {
				cfg_if::cfg_if! {
					if #[cfg(not(feature = "runtime-benchmarks"))] {
						return Err(Error::<T>::ConvertFromNotEnabled)?;
					} else {
						return Ok(().into())
					}
				}
			};
			if &expected_sender != message.sender() {
				return Err(Error::<T>::InvalidSender)?;
			}
			let decoded_message =
				ConversionMessageFor::<T>::decode(&mut message.payload().as_slice())
					.map_err(|_| Error::<T>::DecodingFailure)?;
			Self::process_conversion(decoded_message)?;
			Ok(().into())
		}
	}
}
