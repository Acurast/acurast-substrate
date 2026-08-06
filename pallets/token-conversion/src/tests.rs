use acurast_common::Slashable;
use frame_support::{
	assert_err, assert_ok,
	traits::{
		fungible::{Inspect, InspectHold, Mutate, MutateHold},
		tokens::{Fortitude, Preservation},
		Imbalance,
	},
};
use sp_runtime::traits::{AccountIdConversion, Zero};

use crate::{mock::*, BalanceFor, Config, Conversion, Error, HoldReason, LockedConversion};

/// The former free-balance buffer left un-held during conversion. The conversion
/// processing path (and its `Config::Liquidity`) has been removed, but the tests
/// keep mirroring that buffer so the locked amounts match the historical values.
const LIQUIDITY: Balance = UNIT / 100;

fn account_id() -> AccountId {
	aid(0)
}

fn aid(index: u8) -> AccountId {
	[index; 32].into()
}

/// Establishes a locked conversion for `account_id()` directly.
///
/// The on-chain conversion-processing path has been removed, so this mirrors the
/// state the former `process_conversion` produced: the account is funded from the
/// pallet pot and everything above the liquidity buffer is held and recorded as a
/// lock. This lets us keep exercising `unlock`/`slash` against realistic state.
fn setup_conversion(amount: BalanceFor<Test>) {
	let pot: AccountId = <Test as Config>::PalletId::get().into_account_truncating();
	assert_ok!(<Balances as Mutate<_>>::mint_into(&pot, 1000 * UNIT));
	assert_ok!(<Balances as Mutate<_>>::transfer(
		&pot,
		&account_id(),
		amount,
		Preservation::Protect,
	));
	let hold_amount = amount.saturating_sub(LIQUIDITY);
	assert_ok!(<Balances as MutateHold<_>>::hold(
		&HoldReason::Conversion.into(),
		&account_id(),
		hold_amount,
	));
	LockedConversion::<Test>::insert(
		account_id(),
		Conversion { amount: hold_amount, lock_start: System::block_number() },
	);
}

#[test]
fn test_unlock_1() {
	ExtBuilder.build().execute_with(|| {
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

#[test]
fn test_unlock_2() {
	ExtBuilder.build().execute_with(|| {
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
		setup_conversion(UNIT);

		let locked_balance = UNIT.saturating_sub(LIQUIDITY);
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
fn test_slash() {
	ExtBuilder.build().execute_with(|| {
		let amount = 100 * UNIT;
		setup_conversion(amount);

		let current_block = <Test as Config>::MinLockDuration::get().saturating_sub(1);
		System::set_block_number(current_block.into());

		let account_id = account_id();

		let conversion = LockedConversion::<Test>::get(&account_id).expect("Conversion is present");
		let locked_amount = amount - LIQUIDITY;
		assert_eq!(conversion.amount, locked_amount);

		let slash_amount = 10 * UNIT;
		let credit = AcurastTokenConversion::slash(&account_id, slash_amount).expect("Slash works");
		assert_eq!(credit.peek(), slash_amount);

		let conversion = LockedConversion::<Test>::get(&account_id).expect("Conversion is present");
		assert_eq!(conversion.amount, locked_amount - slash_amount);

		assert_eq!(
			conversion.amount,
			<Balances as InspectHold<AccountId>>::balance_on_hold(
				&HoldReason::Conversion.into(),
				&account_id
			)
		);
	});
}

#[test]
fn test_slash_2() {
	ExtBuilder.build().execute_with(|| {
		let amount = 100 * UNIT;
		setup_conversion(amount);

		let current_block = <Test as Config>::MinLockDuration::get().saturating_sub(1);
		System::set_block_number(current_block.into());

		let account_id = account_id();

		let conversion = LockedConversion::<Test>::get(&account_id).expect("Conversion is present");
		let locked_amount = amount - LIQUIDITY;
		assert_eq!(conversion.amount, locked_amount);

		let slash_amount = 110 * UNIT;
		let credit = AcurastTokenConversion::slash(&account_id, slash_amount).expect("Slash works");
		assert_eq!(credit.peek(), locked_amount);
		assert_eq!(LockedConversion::<Test>::get(&account_id), None);
		assert_eq!(
			0,
			<Balances as InspectHold<AccountId>>::balance_on_hold(
				&HoldReason::Conversion.into(),
				&account_id
			)
		);
	});
}
