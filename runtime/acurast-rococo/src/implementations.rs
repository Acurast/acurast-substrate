use crate::*;
use frame_support::traits::{
	tokens::WithdrawReasons, Currency, ExistenceRequirement::KeepAlive, LockIdentifier,
	LockableCurrency,
};
use pallet_acurast_vesting::Vesting;
use pallet_parachain_staking::{hooks::StakingHooks, BalanceOf, RoundIndex, Stake, StakeOf};

const PALLET_ID: PalletId = PalletId(*b"vestlock");

pub fn vesting_locks_account() -> <Runtime as frame_system::Config>::AccountId {
	PALLET_ID.into_account_truncating()
}

pub struct StakingOverVesting;

pub const VESTING_LOCK_ID: LockIdentifier = *b"vestingl";

impl StakingHooks<Runtime> for StakingOverVesting {
	fn get_collator_stakable_balance(
		acc: &<Runtime as frame_system::Config>::AccountId,
	) -> Result<BalanceOf<Runtime>, DispatchError> {
		Ok(crate::AcurastVesting::vester_states(acc)
			.ok_or(DispatchError::Other("Need to vest before staking as collator"))?
			.stake
			.into())
	}

	fn payout_collator_reward(
		_for_round: RoundIndex,
		_collator: &<Runtime as frame_system::Config>::AccountId,
		reward: BalanceOf<Runtime>,
	) -> Weight {
		// don't use result => uses default handling when imbalance is dropped (adjusting total issuance)
		let _imbalance = <crate::Balances as Currency<
			<Runtime as frame_system::Config>::AccountId,
		>>::deposit_creating(&vesting_locks_account(), reward);

		<weight::pallet_balances::WeightInfo<Runtime> as pallet_balances::WeightInfo>::transfer_keep_alive()
	}

	fn power(
		acc: &<Runtime as frame_system::Config>::AccountId,
		stake: BalanceOf<Runtime>,
	) -> Result<StakeOf<Runtime>, DispatchError> {
		let state = crate::AcurastVesting::vester_states(acc)
			.ok_or(DispatchError::Other("Need to vest before staking as collator"))?;
		Ok(Stake::new(
			stake,
			crate::AcurastVesting::calculate_power(&Vesting {
				stake,
				locking_period: state.locking_period,
			})
			.map_err(|_| DispatchError::Other("Could not calculate power"))?,
		))
	}

	fn compound(
		target: &<Runtime as frame_system::Config>::AccountId,
		more: BalanceOf<Runtime>,
	) -> Result<(), DispatchError> {
		crate::AcurastVesting::compound(target, more)
			.map_err(|_| DispatchError::Other("Could not compound"))
	}
}

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
		if crate::ParachainStaking::is_candidate(&target) {
			return Err(DispatchError::Other("Cannot unvest whilst being a candidate for staking"))
		}

		if crate::ParachainStaking::is_delegator(&target) {
			return Err(DispatchError::Other("Cannot unvest while being delegator"))
		}

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

	fn power_decreased(target: &AccountId, perbill: Perbill) -> Result<(), DispatchError> {
		crate::ParachainStaking::power_decreased(target, perbill)?;
		Ok(())
	}

	fn power_increased(
		target: &AccountId,
		reciprocal_perbill: Perbill,
	) -> Result<(), DispatchError> {
		crate::ParachainStaking::power_increased(target, reciprocal_perbill)?;
		Ok(())
	}
}
