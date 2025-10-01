use frame_benchmarking::v2::*;
use frame_support::{
	dispatch::DispatchResult, pallet_prelude::*, sp_runtime::traits::AccountIdConversion,
	traits::fungible::Mutate,
};
use frame_system::RawOrigin;
use sp_std::prelude::*;

use crate::{BalanceFor, Call, Config, ConversionMessageFor, Pallet};

#[benchmarks(
	where BalanceFor<T>: IsType<u128> + From<u64>,
)]
mod benches {
	use super::{Pallet as TokenConversion, *};
	use sp_runtime::Saturating;

	// helper inside the benchmark module so `T` is injected by the macro
	fn mint_to<T: Config>(who: &T::AccountId, amount: BalanceFor<T>) {
		let _ = <<T as crate::Config>::Currency as Mutate<T::AccountId>>::mint_into(who, amount);
	}

	/// convert
	#[benchmark]
	fn convert() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = account("origin", 0, 0);
		let pallet_account: T::AccountId = T::PalletId::get().into_account_truncating();

		let initial_balance: BalanceFor<T> = 1_000_000_000_000u128.into();

		mint_to::<T>(&caller, initial_balance);
		mint_to::<T>(&pallet_account, initial_balance);

		// Ensure enabled
		let _ = TokenConversion::<T>::set_enabled(RawOrigin::Root.into(), true);

		let fee: BalanceFor<T> = 1_000_000_000u128.into();

		// measured extrinsic call — **bare** call expression, first arg must be origin
		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), fee);

		Ok(())
	}

	/// unlock (single benchmark that prepares state so unlock succeeds)
	#[benchmark]
	fn unlock() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let pallet_account: T::AccountId = T::PalletId::get().into_account_truncating();

		let initial_balance: BalanceFor<T> = 1_000_000_000_000u128.into();
		mint_to::<T>(&pallet_account, initial_balance);

		// create and process conversion
		let msg = ConversionMessageFor::<T> {
			account: caller.clone(),
			amount: T::Liquidity::get().saturating_mul(From::from(2u32)),
		};
		let _: DispatchResult = TokenConversion::<T>::process_conversion(msg);

		// compute unlock block from stored lock
		let lock = TokenConversion::<T>::locked_conversion(&caller)
			.ok_or(BenchmarkError::Stop("locked conversion must exist after processing"))?;
		let unlock_after = lock
			.lock_start
			.saturating_add(T::MaxLockDuration::get().saturating_sub(1000u32.into()));

		frame_system::Pallet::<T>::set_block_number(unlock_after);

		// measured extrinsic (bare call)
		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()));

		Ok(())
	}

	/// retry_convert
	#[benchmark]
	fn retry_convert() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let pallet_account: T::AccountId = T::PalletId::get().into_account_truncating();

		let initial_balance: BalanceFor<T> = 1_000_000_000_000u128.into();
		mint_to::<T>(&caller, initial_balance);
		mint_to::<T>(&pallet_account, initial_balance);

		// enable pallet
		let _ = TokenConversion::<T>::set_enabled(RawOrigin::Root.into(), true);

		let fee: BalanceFor<T> = 1_000_000_000u128.into();

		// create conversion
		_ = TokenConversion::<T>::convert(RawOrigin::Signed(caller.clone()).into(), fee);

		frame_system::Pallet::<T>::set_block_number(
			T::ConvertTTL::get().saturating_add(100u32.into()),
		);

		// measured extrinsic
		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), fee);

		Ok(())
	}

	/// retry_convert
	#[benchmark]
	fn retry_convert_for() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let account: T::AccountId = account("target", 0, 0);
		let pallet_account: T::AccountId = T::PalletId::get().into_account_truncating();

		let initial_balance: BalanceFor<T> = 1_000_000_000_000u128.into();
		mint_to::<T>(&caller, initial_balance);
		mint_to::<T>(&account, initial_balance);
		mint_to::<T>(&pallet_account, initial_balance);

		// enable pallet
		let _ = TokenConversion::<T>::set_enabled(RawOrigin::Root.into(), true);

		let fee: BalanceFor<T> = 1_000_000_000u128.into();

		// create conversion
		_ = TokenConversion::<T>::convert(RawOrigin::Signed(account.clone()).into(), fee);

		frame_system::Pallet::<T>::set_block_number(
			T::ConvertTTL::get().saturating_add(100u32.into()),
		);

		// measured extrinsic
		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), account, fee);

		Ok(())
	}

	/// retry_process_conversion
	#[benchmark]
	fn retry_process_conversion() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let pallet_account: T::AccountId = T::PalletId::get().into_account_truncating();

		// insert an unprocessed conversion for `target`
		let msg = ConversionMessageFor::<T> {
			account: caller.clone(),
			amount: T::Liquidity::get().saturating_mul(From::from(2u32)),
		};
		_ = TokenConversion::<T>::process_conversion(msg);

		let initial_balance: BalanceFor<T> = 1_000_000_000_000u128.into();
		mint_to::<T>(&pallet_account, initial_balance);

		// measured extrinsic
		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()));

		Ok(())
	}

	/// retry_process_conversion_for
	#[benchmark]
	fn retry_process_conversion_for() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let target: T::AccountId = account("target", 0, 0);
		let pallet_account: T::AccountId = T::PalletId::get().into_account_truncating();

		// insert an unprocessed conversion for `target`
		let msg = ConversionMessageFor::<T> {
			account: target.clone(),
			amount: T::Liquidity::get().saturating_mul(From::from(2u32)),
		};
		_ = TokenConversion::<T>::process_conversion(msg);

		let initial_balance: BalanceFor<T> = 1_000_000_000_000u128.into();
		mint_to::<T>(&pallet_account, initial_balance);

		// measured extrinsic
		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), target.clone());

		Ok(())
	}

	/// set_enabled
	#[benchmark]
	fn set_enabled() -> Result<(), BenchmarkError> {
		// measured extrinsic
		#[extrinsic_call]
		_(RawOrigin::Root, true);

		Ok(())
	}
}
