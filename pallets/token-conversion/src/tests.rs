use frame_support::{
	assert_err, assert_ok,
	traits::{
		fungible::{Inspect, InspectHold, Mutate},
		tokens::{Fortitude, Preservation},
		LockableCurrency, WithdrawReasons,
	},
};
use sp_runtime::traits::{AccountIdConversion, Zero};

use crate::{mock::*, BalanceFor, Config, ConversionMessageFor, Error, HoldReason};

fn account_id() -> AccountId {
	aid(0)
}

fn aid(index: u8) -> AccountId {
	[index; 32].into()
}

fn enable() {
	assert_ok!(AcurastTokenConversion::set_enabled(RuntimeOrigin::root(), true));
}

fn setup_conversion(amount: BalanceFor<Test>) {
	assert_ok!(<Balances as Mutate<_>>::mint_into(
		&<Test as Config>::PalletId::get().into_account_truncating(),
		1000 * UNIT
	));
	assert_ok!(AcurastTokenConversion::process_conversion(ConversionMessageFor::<Test> {
		account: account_id(),
		amount,
	}));
}

#[test]
fn test_convert() {
	ExtBuilder.build().execute_with(|| {
		enable();
		assert_ok!(<Balances as Mutate<_>>::mint_into(&account_id(), 205 * UNIT));
		let reducible_balance = <Balances as Inspect<_>>::reducible_balance(
			&account_id(),
			Preservation::Preserve,
			Fortitude::Polite,
		);
		let fee = UNIT / 10;
		let liquidity = <Test as Config>::Liquidity::get();
		assert_ok!(AcurastTokenConversion::convert(RuntimeOrigin::signed(account_id()), fee));
		assert_eq!(
			events().last(),
			Some(&RuntimeEvent::AcurastTokenConversion(crate::Event::ConversionInitiated {
				account: account_id(),
				amount: reducible_balance - fee - liquidity,
			}))
		);

		assert_eq!(
			Balances::reducible_balance(&account_id(), Preservation::Preserve, Fortitude::Polite),
			liquidity
		);
	});
}

#[test]
fn test_unlock_1() {
	ExtBuilder.build().execute_with(|| {
		enable();
		setup_conversion(UNIT);

		System::set_block_number(<Test as Config>::MaxLockDuration::get().saturating_add(2).into());

		assert_ok!(AcurastTokenConversion::unlock(RuntimeOrigin::signed(account_id())));

		assert_eq!(
			events().last(),
			Some(&RuntimeEvent::AcurastTokenConversion(crate::Event::ConversionUnlocked {
				account: account_id()
			}))
		);

		let reducible_balance =
			Balances::reducible_balance(&account_id(), Preservation::Expendable, Fortitude::Polite);
		assert_eq!(reducible_balance, UNIT);

		let on_hold_balance =
			Balances::balance_on_hold(&HoldReason::Conversion.into(), &account_id());
		assert!(on_hold_balance.is_zero());
	});
}

/// Test that conversion hold works even when there's an existing lock (via LockableCurrency)
/// covering the account's total balance.
#[test]
fn test_unlock_with_existing_lock() {
	ExtBuilder.build().execute_with(|| {
		enable();
		const LOCK_ID: [u8; 8] = *b"testlock";

		// First, mint funds to the pallet account and the user account
		assert_ok!(<Balances as Mutate<_>>::mint_into(
			&<Test as Config>::PalletId::get().into_account_truncating(),
			1000 * UNIT
		));
		assert_ok!(<Balances as Mutate<_>>::mint_into(&account_id(), UNIT));

		// Set up a lock covering the entire balance
		<Balances as LockableCurrency<_>>::set_lock(
			LOCK_ID,
			&account_id(),
			UNIT,
			WithdrawReasons::all(),
		);

		// Process conversion - this should still be able to hold funds despite the lock
		assert_ok!(AcurastTokenConversion::process_conversion(ConversionMessageFor::<Test> {
			account: account_id(),
			amount: UNIT,
		}));

		// Verify the hold is in place
		let expected_hold = UNIT.saturating_sub(<Test as Config>::Liquidity::get());
		let on_hold_balance =
			Balances::balance_on_hold(&HoldReason::Conversion.into(), &account_id());
		assert_eq!(on_hold_balance, expected_hold);

		// Verify conversion is locked
		assert!(AcurastTokenConversion::locked_conversion(account_id()).is_some());

		// Move to after max lock duration
		System::set_block_number(<Test as Config>::MaxLockDuration::get().saturating_add(2).into());

		// Unlock should work
		assert_ok!(AcurastTokenConversion::unlock(RuntimeOrigin::signed(account_id())));

		assert_eq!(
			events().last(),
			Some(&RuntimeEvent::AcurastTokenConversion(crate::Event::ConversionUnlocked {
				account: account_id()
			}))
		);

		// Hold should be released
		let on_hold_balance =
			Balances::balance_on_hold(&HoldReason::Conversion.into(), &account_id());
		assert!(on_hold_balance.is_zero());

		// Total balance should be 2*UNIT (original + converted)
		let total_balance = Balances::total_balance(&account_id());
		assert_eq!(total_balance, 2 * UNIT);
	});
}

#[test]
fn test_unlock_2() {
	ExtBuilder.build().execute_with(|| {
		enable();
		setup_conversion(UNIT);

		let current_block = <Test as Config>::MinLockDuration::get().saturating_add(1);
		System::set_block_number(current_block.into());

		assert_ok!(AcurastTokenConversion::unlock(RuntimeOrigin::signed(account_id())));

		let post_pot_balance = <Test as Config>::Currency::total_balance(
			&<Test as Config>::PalletId::get().into_account_truncating(),
		);
		let post_reducible_balance =
			Balances::reducible_balance(&account_id(), Preservation::Expendable, Fortitude::Polite);

		assert_eq!(
			events().last(),
			Some(&RuntimeEvent::AcurastTokenConversion(crate::Event::ConversionUnlocked {
				account: account_id()
			}))
		);

		assert_eq!(post_reducible_balance, 71875);
		assert_eq!(post_pot_balance, 999928125);

		assert!(Balances::balance_on_hold(&HoldReason::Conversion.into(), &account_id()).is_zero());
	});
}

#[test]
fn test_unlock_3() {
	ExtBuilder.build().execute_with(|| {
		enable();
		setup_conversion(UNIT);

		let locked_balance = UNIT.saturating_sub(<Test as Config>::Liquidity::get());
		let pre_pot_balance = <Test as Config>::Currency::total_balance(
			&<Test as Config>::PalletId::get().into_account_truncating(),
		);
		let pre_reducible_balance =
			Balances::reducible_balance(&account_id(), Preservation::Expendable, Fortitude::Polite);

		System::set_block_number(
			<Test as Config>::MaxLockDuration::get()
				.saturating_div(2)
				.saturating_add(1)
				.into(),
		);

		assert_ok!(AcurastTokenConversion::unlock(RuntimeOrigin::signed(account_id())));

		let post_pot_balance = <Test as Config>::Currency::total_balance(
			&<Test as Config>::PalletId::get().into_account_truncating(),
		);
		let post_reducible_balance =
			Balances::reducible_balance(&account_id(), Preservation::Expendable, Fortitude::Polite);

		assert_eq!(
			events().last(),
			Some(&RuntimeEvent::AcurastTokenConversion(crate::Event::ConversionUnlocked {
				account: account_id()
			}))
		);

		// With holds, the pre_reducible_balance is lower by ED when there's a hold, so we need to add ED
		assert_eq!(
			pre_reducible_balance
				.saturating_add(locked_balance.saturating_div(2))
				.saturating_add(EXISTENTIAL_DEPOSIT),
			post_reducible_balance
		);
		assert_eq!(
			pre_pot_balance.saturating_add(locked_balance.saturating_div(2)),
			post_pot_balance
		);

		assert!(Balances::balance_on_hold(&HoldReason::Conversion.into(), &account_id()).is_zero());
	});
}

#[test]
fn test_unlock_4() {
	ExtBuilder.build().execute_with(|| {
		enable();
		setup_conversion(UNIT);

		let current_block = <Test as Config>::MinLockDuration::get().saturating_sub(1);
		System::set_block_number(current_block.into());

		assert_err!(
			AcurastTokenConversion::unlock(RuntimeOrigin::signed(account_id())),
			Error::<Test>::CannotUnlockYet
		);
	});
}

#[test]
fn test_retry_convert() {
	ExtBuilder.build().execute_with(|| {
		enable();
		assert_ok!(<Balances as Mutate<_>>::mint_into(&account_id(), 205 * UNIT));
		assert_ok!(<Balances as Mutate<_>>::mint_into(&aid(1), UNIT));
		let fee = UNIT / 1000;

		assert_err!(
			AcurastTokenConversion::retry_convert(RuntimeOrigin::signed(account_id()), fee),
			Error::<Test>::ConversionLockNotFound
		);

		assert_ok!(AcurastTokenConversion::convert(RuntimeOrigin::signed(account_id()), fee));
		assert_ok!(AcurastTokenConversion::retry_convert(RuntimeOrigin::signed(account_id()), fee));

		assert_eq!(
			events().last(),
			Some(&RuntimeEvent::AcurastTokenConversion(crate::Event::ConversionRetried {
				account: account_id()
			}))
		);

		assert_ok!(AcurastTokenConversion::retry_convert_for(
			RuntimeOrigin::signed(aid(1)),
			account_id(),
			fee
		));

		assert_eq!(
			events().last(),
			Some(&RuntimeEvent::AcurastTokenConversion(crate::Event::ConversionRetried {
				account: account_id()
			}))
		);
	});
}

#[test]
fn test_retry_process_conversion() {
	ExtBuilder.build().execute_with(|| {
		enable();

		assert_err!(
			AcurastTokenConversion::retry_process_conversion(RuntimeOrigin::signed(account_id())),
			Error::<Test>::ConversionLockNotFound
		);

		assert_ok!(AcurastTokenConversion::process_conversion(ConversionMessageFor::<Test> {
			account: account_id(),
			amount: UNIT,
		}));

		assert!(AcurastTokenConversion::unprocessed_conversion(account_id()).is_some());

		assert_ok!(<Balances as Mutate<_>>::mint_into(
			&<Test as Config>::PalletId::get().into_account_truncating(),
			1000 * UNIT
		));

		assert_ok!(AcurastTokenConversion::retry_process_conversion(RuntimeOrigin::signed(
			account_id()
		)));

		assert_eq!(
			events().last(),
			Some(&RuntimeEvent::AcurastTokenConversion(crate::Event::ConversionProcessed {
				account: account_id(),
				amount: UNIT,
			}))
		);

		assert!(AcurastTokenConversion::locked_conversion(account_id()).is_some());
	});
}

#[test]
fn test_retry_process_conversion_for() {
	ExtBuilder.build().execute_with(|| {
		enable();

		assert_err!(
			AcurastTokenConversion::retry_process_conversion_for(
				RuntimeOrigin::signed(aid(1)),
				account_id()
			),
			Error::<Test>::ConversionLockNotFound
		);

		assert_ok!(AcurastTokenConversion::process_conversion(ConversionMessageFor::<Test> {
			account: account_id(),
			amount: UNIT,
		}));

		assert!(AcurastTokenConversion::unprocessed_conversion(account_id()).is_some());

		assert_ok!(<Balances as Mutate<_>>::mint_into(
			&<Test as Config>::PalletId::get().into_account_truncating(),
			1000 * UNIT
		));

		assert_ok!(AcurastTokenConversion::retry_process_conversion_for(
			RuntimeOrigin::signed(aid(1)),
			account_id()
		));

		assert_eq!(
			events().last(),
			Some(&RuntimeEvent::AcurastTokenConversion(crate::Event::ConversionProcessed {
				account: account_id(),
				amount: UNIT,
			}))
		);

		assert!(AcurastTokenConversion::locked_conversion(account_id()).is_some());
	});
}
