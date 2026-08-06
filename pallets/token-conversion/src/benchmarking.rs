use frame_benchmarking::v2::*;
use frame_support::{
	pallet_prelude::*,
	traits::fungible::{Mutate, MutateHold},
};
use frame_system::{pallet_prelude::BlockNumberFor, RawOrigin};
use sp_std::prelude::*;

use crate::{BalanceFor, Call, Config, Conversion, HoldReason, LockedConversion, Pallet};

#[benchmarks(
	where BalanceFor<T>: IsType<u128> + From<u64>,
)]
mod benches {
	use super::*;
	use sp_runtime::Saturating;

	// helper inside the benchmark module so `T` is injected by the macro
	fn mint_to<T: Config>(who: &T::AccountId, amount: BalanceFor<T>) {
		let _ = <<T as crate::Config>::Currency as Mutate<T::AccountId>>::mint_into(who, amount);
	}

	/// Establishes a locked conversion for `who` directly.
	///
	/// The on-chain conversion-processing path has been removed; this mirrors the
	/// state the former `process_conversion` produced so `unlock` can be measured.
	fn setup_lock<T: Config>(who: &T::AccountId) -> BlockNumberFor<T>
	where
		BalanceFor<T>: IsType<u128>,
	{
		// mint enough to keep a free buffer above the held amount
		mint_to::<T>(who, 1_000_000_000_000u128.into());
		let hold_amount: BalanceFor<T> = 900_000_000_000u128.into();
		let _ = <<T as crate::Config>::Currency as MutateHold<T::AccountId>>::hold(
			&HoldReason::Conversion.into(),
			who,
			hold_amount,
		);
		let lock_start = frame_system::Pallet::<T>::block_number();
		<LockedConversion<T>>::insert(who, Conversion { amount: hold_amount, lock_start });
		lock_start
	}

	/// unlock (single benchmark that prepares state so unlock succeeds)
	#[benchmark]
	fn unlock() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();

		let lock_start = setup_lock::<T>(&caller);

		let unlock_after =
			lock_start.saturating_add(T::MaxLockDuration::get().saturating_sub(1000u32.into()));
		frame_system::Pallet::<T>::set_block_number(unlock_after);

		// measured extrinsic (bare call)
		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()));

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

	/// deny_source
	///
	/// Previously unbenchmarked while its `#[pallet::weight]` borrowed
	/// `retry_process_conversion_for()` — one of the disabled stubs, which does no storage work at
	/// all. This call writes to `DeniedSource`, so it was charged less than it costs.
	#[benchmark]
	fn deny_source() -> Result<(), BenchmarkError> {
		let account: T::AccountId = whitelisted_caller();

		// measured extrinsic
		#[extrinsic_call]
		_(RawOrigin::Root, account.clone(), true);

		assert_eq!(crate::DeniedSource::<T>::get(&account), Some(true));

		Ok(())
	}
}
