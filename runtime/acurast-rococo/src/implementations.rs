use crate::*;
use frame_support::traits::{
	tokens::WithdrawReasons, Currency, ExistenceRequirement::KeepAlive, LockIdentifier,
	LockableCurrency,
};
use pallet_acurast_vesting::VestingBalance;
use sp_runtime::traits::AccountIdConversion;

const PALLET_ID: PalletId = PalletId(*b"vestlock");

pub fn vesting_locks_account() -> <Runtime as frame_system::Config>::AccountId {
	PALLET_ID.into_account_truncating()
}

pub struct StakingOverVesting;

pub const VESTING_LOCK_ID: LockIdentifier = *b"vestingl";

pub type BalanceOf<T> = <T as pallet_acurast_vesting::Config>::Balance;

impl VestingBalance<AccountId, Balance> for StakingOverVesting {
	fn lock_stake(target: &AccountId, stake: Balance) -> Result<(), DispatchError> {
		<crate::Balances as LockableCurrency<<Runtime as frame_system::Config>::AccountId>>::set_lock(
			VESTING_LOCK_ID,
			&target,
			stake,
			WithdrawReasons::all(),
		);
		Ok(())
	}

	fn pay_accrued(target: &AccountId, accrued: Balance) -> Result<(), DispatchError> {
		<crate::Balances as Currency<<Runtime as frame_system::Config>::AccountId>>::transfer(
			&vesting_locks_account(),
			target,
			accrued,
			KeepAlive,
		)?;
		Ok(())
	}

	fn pay_kicker(target: &AccountId, accrued: Balance) -> Result<(), DispatchError> {
		Self::pay_accrued(target, accrued)
	}

	fn unlock_stake(target: &AccountId, _stake: Balance) -> Result<(), DispatchError> {
		<crate::Balances as LockableCurrency<<Runtime as frame_system::Config>::AccountId>>::remove_lock(
			VESTING_LOCK_ID,
			&target,
		);
		Ok(())
	}

	fn adjust_lock(acc: &<Runtime as frame_system::Config>::AccountId, stake: BalanceOf<Runtime>) {
		<crate::Balances as LockableCurrency<<Runtime as frame_system::Config>::AccountId>>::set_lock(
			VESTING_LOCK_ID,
			acc,
			stake,
			WithdrawReasons::all(),
		);
	}

	fn power_decreased(_target: &AccountId, _perbill: Perbill) -> Result<(), DispatchError> {
		Ok(())
	}

	fn power_increased(
		_target: &AccountId,
		_reciprocal_perbill: Perbill,
	) -> Result<(), DispatchError> {
		Ok(())
	}
}
