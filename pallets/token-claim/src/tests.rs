use frame_support::{
	assert_err, assert_ok,
	traits::fungible::{Inspect, Mutate},
};

use crate::{mock::*, ClaimProof, Config, Error};

// ============================================================================
// Happy path tests
// ============================================================================

#[test]
fn test_claim_to_same_account() {
	ExtBuilder.build().execute_with(|| {
		let claim_amount: Balance = 100 * UNIT;
		let initial_funds = claim_amount * 10;
		assert_ok!(<Balances as Mutate<_>>::mint_into(
			&<Test as Config>::Funder::get(),
			initial_funds
		));
		let (_, account) = generate_pair_account("User");
		let (signer, _) = generate_pair_account("Bob");
		let signature = generate_signature(&signer, &account, claim_amount);
		let proof = ClaimProof::new(claim_amount, signature);

		let claim_block = System::block_number();
		assert_ok!(AcurastTokenClaim::claim(
			RuntimeOrigin::signed(account.clone()),
			proof,
			account.clone()
		));

		// Check ClaimedV2 event
		assert_eq!(
			events().last(),
			Some(&RuntimeEvent::AcurastTokenClaim(crate::Event::ClaimedV2 {
				claimer: account.clone(),
				destination: account.clone(),
				amount: claim_amount
			}))
		);

		// With Funder approach, funds stay on Funder until vest
		assert_eq!(Balances::balance(&<Test as Config>::Funder::get()), initial_funds);
		assert_eq!(Balances::balance(&account), 0);

		// Check vesting schedule is stored with correct fields (keyed by destination, claimer)
		let vesting = AcurastTokenClaim::vesting(&account, &account);
		assert!(vesting.is_some());
		let schedule = vesting.unwrap();
		assert_eq!(schedule.remaining, claim_amount);
		assert_eq!(schedule.claimer, account);
		assert_eq!(schedule.starting_block, claim_block);
		assert_eq!(schedule.latest_vest, claim_block);
		assert!(schedule.per_block > 0);

		// Check claimed storage
		let claimed = AcurastTokenClaim::claimed(&account);
		assert!(claimed.is_some());
		let processed = claimed.unwrap();
		assert_eq!(processed.destination, account);
		assert_eq!(processed.proof.amount, claim_amount);
	});
}

#[test]
fn test_claim_to_different_destination() {
	ExtBuilder.build().execute_with(|| {
		let claim_amount: Balance = 100 * UNIT;
		let initial_funds = claim_amount * 10;
		assert_ok!(<Balances as Mutate<_>>::mint_into(
			&<Test as Config>::Funder::get(),
			initial_funds
		));
		let (_, claimer) = generate_pair_account("Claimer");
		let (_, destination) = generate_pair_account("Destination");
		let (signer, _) = generate_pair_account("Bob");
		let signature = generate_signature(&signer, &claimer, claim_amount);
		let proof = ClaimProof::new(claim_amount, signature);

		let claim_block = System::block_number();
		assert_ok!(AcurastTokenClaim::claim(
			RuntimeOrigin::signed(claimer.clone()),
			proof,
			destination.clone()
		));

		// Check ClaimedV2 event has correct account and destination
		assert_eq!(
			events().last(),
			Some(&RuntimeEvent::AcurastTokenClaim(crate::Event::ClaimedV2 {
				claimer: claimer.clone(),
				destination: destination.clone(),
				amount: claim_amount
			}))
		);

		// Vesting schedule is stored under (destination, claimer)
		assert!(AcurastTokenClaim::vesting(&claimer, &claimer).is_none());
		let vesting = AcurastTokenClaim::vesting(&destination, &claimer);
		assert!(vesting.is_some());
		let schedule = vesting.unwrap();
		assert_eq!(schedule.remaining, claim_amount);
		assert_eq!(schedule.claimer, claimer.clone());
		assert_eq!(schedule.starting_block, claim_block);
		assert_eq!(schedule.latest_vest, claim_block);

		// Claimed storage is stored under claimer
		let claimed = AcurastTokenClaim::claimed(&claimer);
		assert!(claimed.is_some());
		assert_eq!(claimed.unwrap().destination, destination);
	});
}

#[test]
fn test_vest_partial() {
	ExtBuilder.build().execute_with(|| {
		let claim_amount: Balance = 100 * UNIT;
		let initial_funds = claim_amount * 10;
		assert_ok!(<Balances as Mutate<_>>::mint_into(
			&<Test as Config>::Funder::get(),
			initial_funds
		));
		let (_, account) = generate_pair_account("User");
		let (signer, _) = generate_pair_account("Bob");
		let signature = generate_signature(&signer, &account, claim_amount);
		let proof = ClaimProof::new(claim_amount, signature);

		let claim_block = System::block_number();
		assert_ok!(AcurastTokenClaim::claim(
			RuntimeOrigin::signed(account.clone()),
			proof,
			account.clone()
		));

		// Move forward in time to vest some tokens (half the duration)
		let vesting_duration = <Test as Config>::VestingDuration::get();
		let vest_block = (vesting_duration / 2) as u64;
		System::set_block_number(vest_block);

		// Vest to unlock some tokens (claimer defaults to origin)
		assert_ok!(AcurastTokenClaim::vest(RuntimeOrigin::signed(account.clone()), None, None));

		// Check that some tokens were transferred to the user
		let user_balance = Balances::balance(&account);
		assert!(user_balance > 0);
		assert!(user_balance < claim_amount);

		// Check vesting schedule is updated
		let vesting = AcurastTokenClaim::vesting(&account, &account).unwrap();
		// starting_block should remain unchanged
		assert_eq!(vesting.starting_block, claim_block);
		// latest_vest should be updated to current block
		assert_eq!(vesting.latest_vest, vest_block);
		assert!(vesting.remaining < claim_amount);
		assert!(vesting.remaining > 0);

		// Check Vested event
		assert!(events().iter().any(|e| matches!(
			e,
			RuntimeEvent::AcurastTokenClaim(crate::Event::Vested { claimer: c, destination: d, remaining: r })
				if d == &account && c == &account && *r == vesting.remaining
		)));

		// Funder balance should have decreased
		assert!(Balances::balance(&<Test as Config>::Funder::get()) < initial_funds);
	});
}

#[test]
fn test_vest_complete() {
	ExtBuilder.build().execute_with(|| {
		let claim_amount: Balance = 100 * UNIT;
		let initial_funds = claim_amount * 10;
		assert_ok!(<Balances as Mutate<_>>::mint_into(
			&<Test as Config>::Funder::get(),
			initial_funds
		));
		let (_, account) = generate_pair_account("User");
		let (signer, _) = generate_pair_account("Bob");
		let signature = generate_signature(&signer, &account, claim_amount);
		let proof = ClaimProof::new(claim_amount, signature);
		assert_ok!(AcurastTokenClaim::claim(
			RuntimeOrigin::signed(account.clone()),
			proof,
			account.clone()
		));

		// Move forward well past vesting duration to ensure full vesting
		let vesting_duration = <Test as Config>::VestingDuration::get() as u64;
		System::set_block_number(vesting_duration * 2);

		// Vest to unlock all tokens
		assert_ok!(AcurastTokenClaim::vest(RuntimeOrigin::signed(account.clone()), None, None));

		// Check that all tokens were transferred to the user
		assert_eq!(Balances::balance(&account), claim_amount);

		// Check vesting schedule is removed
		assert!(AcurastTokenClaim::vesting(&account, &account).is_none());

		// Check Vested event with remaining = 0
		assert!(events().iter().any(|e| matches!(
			e,
			RuntimeEvent::AcurastTokenClaim(crate::Event::Vested { claimer: c, destination: d, remaining: r })
				if d == &account && c == &account && *r == 0
		)));

		// Funder balance should have decreased by claim_amount
		assert_eq!(
			Balances::balance(&<Test as Config>::Funder::get()),
			initial_funds - claim_amount
		);
	});
}

#[test]
fn test_vest_multiple_times() {
	ExtBuilder.build().execute_with(|| {
		let claim_amount: Balance = 100 * UNIT;
		let initial_funds = claim_amount * 10;
		assert_ok!(<Balances as Mutate<_>>::mint_into(
			&<Test as Config>::Funder::get(),
			initial_funds
		));
		let (_, account) = generate_pair_account("User");
		let (signer, _) = generate_pair_account("Bob");
		let signature = generate_signature(&signer, &account, claim_amount);
		let proof = ClaimProof::new(claim_amount, signature);

		let claim_block = System::block_number();
		assert_ok!(AcurastTokenClaim::claim(
			RuntimeOrigin::signed(account.clone()),
			proof,
			account.clone()
		));

		let vesting_duration = <Test as Config>::VestingDuration::get();

		// First vest at 25%
		let first_vest_block = (vesting_duration / 4) as u64;
		System::set_block_number(first_vest_block);
		assert_ok!(AcurastTokenClaim::vest(RuntimeOrigin::signed(account.clone()), None, None));
		let balance_after_first = Balances::balance(&account);
		assert!(balance_after_first > 0);

		let vesting = AcurastTokenClaim::vesting(&account, &account).unwrap();
		assert_eq!(vesting.starting_block, claim_block); // unchanged
		assert_eq!(vesting.latest_vest, first_vest_block); // updated

		// Second vest at 50%
		let second_vest_block = (vesting_duration / 2) as u64;
		System::set_block_number(second_vest_block);
		assert_ok!(AcurastTokenClaim::vest(RuntimeOrigin::signed(account.clone()), None, None));
		let balance_after_second = Balances::balance(&account);
		assert!(balance_after_second > balance_after_first);

		let vesting = AcurastTokenClaim::vesting(&account, &account).unwrap();
		assert_eq!(vesting.starting_block, claim_block); // still unchanged
		assert_eq!(vesting.latest_vest, second_vest_block); // updated again

		// Third vest at 100%+
		System::set_block_number((vesting_duration * 2) as u64);
		assert_ok!(AcurastTokenClaim::vest(RuntimeOrigin::signed(account.clone()), None, None));
		assert_eq!(Balances::balance(&account), claim_amount);
		assert!(AcurastTokenClaim::vesting(&account, &account).is_none());
	});
}

#[test]
fn test_vest_other_account() {
	ExtBuilder.build().execute_with(|| {
		let claim_amount: Balance = 100 * UNIT;
		let initial_funds = claim_amount * 10;
		assert_ok!(<Balances as Mutate<_>>::mint_into(
			&<Test as Config>::Funder::get(),
			initial_funds
		));
		let (_, account) = generate_pair_account("User");
		let (_, other_account) = generate_pair_account("Other");
		let (signer, _) = generate_pair_account("Bob");
		let signature = generate_signature(&signer, &account, claim_amount);
		let proof = ClaimProof::new(claim_amount, signature);
		assert_ok!(AcurastTokenClaim::claim(
			RuntimeOrigin::signed(account.clone()),
			proof,
			account.clone()
		));

		// Move forward well past vesting duration
		let vesting_duration = <Test as Config>::VestingDuration::get() as u64;
		System::set_block_number(vesting_duration * 2);

		// Another account can vest on behalf of the user by specifying destination and claimer
		assert_ok!(AcurastTokenClaim::vest(
			RuntimeOrigin::signed(other_account.clone()),
			Some(account.clone()), // destination
			Some(account.clone())  // claimer
		));

		// Check that all tokens were transferred to the user (not the caller)
		assert_eq!(Balances::balance(&account), claim_amount);
		assert_eq!(Balances::balance(&other_account), 0);
	});
}

// ============================================================================
// Error case tests
// ============================================================================

#[test]
fn test_error_invalid_claim_wrong_signer() {
	ExtBuilder.build().execute_with(|| {
		let claim_amount: Balance = 100 * UNIT;
		let initial_funds = claim_amount * 10;
		assert_ok!(<Balances as Mutate<_>>::mint_into(
			&<Test as Config>::Funder::get(),
			initial_funds
		));
		let (_, account) = generate_pair_account("User");
		// Use wrong signer (Charlie instead of Bob)
		let (wrong_signer, _) = generate_pair_account("Charlie");
		let signature = generate_signature(&wrong_signer, &account, claim_amount);
		let proof = ClaimProof::new(claim_amount, signature);

		assert_err!(
			AcurastTokenClaim::claim(
				RuntimeOrigin::signed(account.clone()),
				proof,
				account.clone()
			),
			Error::<Test>::InvalidClaim
		);
	});
}

#[test]
fn test_error_invalid_claim_wrong_amount() {
	ExtBuilder.build().execute_with(|| {
		let claim_amount: Balance = 100 * UNIT;
		let wrong_amount: Balance = 200 * UNIT;
		let initial_funds = claim_amount * 10;
		assert_ok!(<Balances as Mutate<_>>::mint_into(
			&<Test as Config>::Funder::get(),
			initial_funds
		));
		let (_, account) = generate_pair_account("User");
		let (signer, _) = generate_pair_account("Bob");
		// Sign for wrong amount
		let signature = generate_signature(&signer, &account, wrong_amount);
		let proof = ClaimProof::new(claim_amount, signature);

		assert_err!(
			AcurastTokenClaim::claim(
				RuntimeOrigin::signed(account.clone()),
				proof,
				account.clone()
			),
			Error::<Test>::InvalidClaim
		);
	});
}

#[test]
fn test_error_invalid_claim_wrong_account() {
	ExtBuilder.build().execute_with(|| {
		let claim_amount: Balance = 100 * UNIT;
		let initial_funds = claim_amount * 10;
		assert_ok!(<Balances as Mutate<_>>::mint_into(
			&<Test as Config>::Funder::get(),
			initial_funds
		));
		let (_, account) = generate_pair_account("User");
		let (_, wrong_account) = generate_pair_account("WrongUser");
		let (signer, _) = generate_pair_account("Bob");
		// Sign for wrong account
		let signature = generate_signature(&signer, &wrong_account, claim_amount);
		let proof = ClaimProof::new(claim_amount, signature);

		assert_err!(
			AcurastTokenClaim::claim(
				RuntimeOrigin::signed(account.clone()),
				proof,
				account.clone()
			),
			Error::<Test>::InvalidClaim
		);
	});
}

#[test]
fn test_error_already_claimed() {
	ExtBuilder.build().execute_with(|| {
		let claim_amount: Balance = 100 * UNIT;
		let initial_funds = claim_amount * 10;
		assert_ok!(<Balances as Mutate<_>>::mint_into(
			&<Test as Config>::Funder::get(),
			initial_funds
		));
		let (_, account) = generate_pair_account("User");
		let (signer, _) = generate_pair_account("Bob");
		let signature = generate_signature(&signer, &account, claim_amount);
		let proof = ClaimProof::new(claim_amount, signature);

		// First claim succeeds
		assert_ok!(AcurastTokenClaim::claim(
			RuntimeOrigin::signed(account.clone()),
			proof.clone(),
			account.clone()
		));

		// Second claim with same proof fails
		assert_err!(
			AcurastTokenClaim::claim(
				RuntimeOrigin::signed(account.clone()),
				proof,
				account.clone()
			),
			Error::<Test>::AlreadyClaimed
		);
	});
}

#[test]
fn test_error_not_vesting() {
	ExtBuilder.build().execute_with(|| {
		let (_, account) = generate_pair_account("User");

		// Try to vest without having a vesting schedule
		assert_err!(
			AcurastTokenClaim::vest(RuntimeOrigin::signed(account.clone()), None, None),
			Error::<Test>::NotVesting
		);

		// Also test with explicit destination and claimer
		let (_, other) = generate_pair_account("Other");
		assert_err!(
			AcurastTokenClaim::vest(
				RuntimeOrigin::signed(account),
				Some(other.clone()),
				Some(other)
			),
			Error::<Test>::NotVesting
		);
	});
}

#[test]
fn test_error_vest_amount_too_low() {
	ExtBuilder.build().execute_with(|| {
		let claim_amount: Balance = 100 * UNIT;
		let initial_funds = claim_amount * 10;
		assert_ok!(<Balances as Mutate<_>>::mint_into(
			&<Test as Config>::Funder::get(),
			initial_funds
		));
		let (_, account) = generate_pair_account("User");
		let (signer, _) = generate_pair_account("Bob");
		let signature = generate_signature(&signer, &account, claim_amount);
		let proof = ClaimProof::new(claim_amount, signature);
		assert_ok!(AcurastTokenClaim::claim(
			RuntimeOrigin::signed(account.clone()),
			proof,
			account.clone()
		));

		// Try to vest immediately (block 1) - vestable amount will be very small
		// Since we're at block 1 and claim happened at block 1, vestable = 0 blocks * per_block = 0
		// This should fail with VestAmountTooLow
		assert_err!(
			AcurastTokenClaim::vest(RuntimeOrigin::signed(account.clone()), None, None),
			Error::<Test>::VestAmountTooLow
		);
	});
}

#[test]
fn test_error_vest_amount_too_low_after_partial_vest() {
	ExtBuilder.build().execute_with(|| {
		let claim_amount: Balance = 100 * UNIT;
		let initial_funds = claim_amount * 10;
		assert_ok!(<Balances as Mutate<_>>::mint_into(
			&<Test as Config>::Funder::get(),
			initial_funds
		));
		let (_, account) = generate_pair_account("User");
		let (signer, _) = generate_pair_account("Bob");
		let signature = generate_signature(&signer, &account, claim_amount);
		let proof = ClaimProof::new(claim_amount, signature);
		assert_ok!(AcurastTokenClaim::claim(
			RuntimeOrigin::signed(account.clone()),
			proof,
			account.clone()
		));

		// Move forward and vest
		let vesting_duration = <Test as Config>::VestingDuration::get();
		System::set_block_number((vesting_duration / 4) as u64);
		assert_ok!(AcurastTokenClaim::vest(RuntimeOrigin::signed(account.clone()), None, None));

		// Try to vest again immediately - no time has passed so vestable = 0
		assert_err!(
			AcurastTokenClaim::vest(RuntimeOrigin::signed(account.clone()), None, None),
			Error::<Test>::VestAmountTooLow
		);
	});
}

#[test]
fn test_error_not_vesting_after_complete() {
	ExtBuilder.build().execute_with(|| {
		let claim_amount: Balance = 100 * UNIT;
		let initial_funds = claim_amount * 10;
		assert_ok!(<Balances as Mutate<_>>::mint_into(
			&<Test as Config>::Funder::get(),
			initial_funds
		));
		let (_, account) = generate_pair_account("User");
		let (signer, _) = generate_pair_account("Bob");
		let signature = generate_signature(&signer, &account, claim_amount);
		let proof = ClaimProof::new(claim_amount, signature);
		assert_ok!(AcurastTokenClaim::claim(
			RuntimeOrigin::signed(account.clone()),
			proof,
			account.clone()
		));

		// Complete vesting
		let vesting_duration = <Test as Config>::VestingDuration::get() as u64;
		System::set_block_number(vesting_duration * 2);
		assert_ok!(AcurastTokenClaim::vest(RuntimeOrigin::signed(account.clone()), None, None));

		// Vesting schedule should be removed
		assert!(AcurastTokenClaim::vesting(&account, &account).is_none());

		// Trying to vest again should fail with NotVesting
		System::set_block_number(vesting_duration * 3);
		assert_err!(
			AcurastTokenClaim::vest(RuntimeOrigin::signed(account.clone()), None, None),
			Error::<Test>::NotVesting
		);
	});
}

// ============================================================================
// Edge case tests
// ============================================================================

#[test]
fn test_claim_minimum_amount() {
	ExtBuilder.build().execute_with(|| {
		// Claim the minimum possible amount (existential deposit)
		let claim_amount: Balance = EXISTENTIAL_DEPOSIT;
		let initial_funds = claim_amount * 10;
		assert_ok!(<Balances as Mutate<_>>::mint_into(
			&<Test as Config>::Funder::get(),
			initial_funds
		));
		let (_, account) = generate_pair_account("User");
		let (signer, _) = generate_pair_account("Bob");
		let signature = generate_signature(&signer, &account, claim_amount);
		let proof = ClaimProof::new(claim_amount, signature);

		assert_ok!(AcurastTokenClaim::claim(
			RuntimeOrigin::signed(account.clone()),
			proof,
			account.clone()
		));

		// Vesting should be stored
		assert!(AcurastTokenClaim::vesting(&account, &account).is_some());
	});
}

#[test]
fn test_claim_large_amount() {
	ExtBuilder.build().execute_with(|| {
		// Claim a very large amount
		let claim_amount: Balance = 1_000_000_000 * UNIT;
		let initial_funds = claim_amount * 2;
		assert_ok!(<Balances as Mutate<_>>::mint_into(
			&<Test as Config>::Funder::get(),
			initial_funds
		));
		let (_, account) = generate_pair_account("User");
		let (signer, _) = generate_pair_account("Bob");
		let signature = generate_signature(&signer, &account, claim_amount);
		let proof = ClaimProof::new(claim_amount, signature);

		assert_ok!(AcurastTokenClaim::claim(
			RuntimeOrigin::signed(account.clone()),
			proof,
			account.clone()
		));

		let vesting = AcurastTokenClaim::vesting(&account, &account).unwrap();
		assert_eq!(vesting.remaining, claim_amount);

		// Complete vest
		let vesting_duration = <Test as Config>::VestingDuration::get() as u64;
		System::set_block_number(vesting_duration * 2);
		assert_ok!(AcurastTokenClaim::vest(RuntimeOrigin::signed(account.clone()), None, None));
		assert_eq!(Balances::balance(&account), claim_amount);
	});
}

#[test]
fn test_vest_exactly_at_end() {
	ExtBuilder.build().execute_with(|| {
		let claim_amount: Balance = 100 * UNIT;
		let initial_funds = claim_amount * 10;
		assert_ok!(<Balances as Mutate<_>>::mint_into(
			&<Test as Config>::Funder::get(),
			initial_funds
		));
		let (_, account) = generate_pair_account("User");
		let (signer, _) = generate_pair_account("Bob");
		let signature = generate_signature(&signer, &account, claim_amount);
		let proof = ClaimProof::new(claim_amount, signature);
		assert_ok!(AcurastTokenClaim::claim(
			RuntimeOrigin::signed(account.clone()),
			proof,
			account.clone()
		));

		// Vest exactly at the end of vesting duration
		let vesting_duration = <Test as Config>::VestingDuration::get() as u64;
		System::set_block_number(1 + vesting_duration);

		assert_ok!(AcurastTokenClaim::vest(RuntimeOrigin::signed(account.clone()), None, None));

		// Due to integer division rounding in per_block calculation, the vestable amount
		// may be less than claim_amount at exactly the vesting duration boundary.
		// The important thing is that we've vested a substantial portion.
		let user_balance = Balances::balance(&account);
		assert!(user_balance > 0);
		// Since per_block = amount / duration, at duration we get approximately amount
		// but rounding means we might get slightly less. The vestable function caps at remaining,
		// so if we still have remaining balance, it means vesting isn't complete yet.
		let vesting = AcurastTokenClaim::vesting(&account, &account);
		if vesting.is_some() {
			// Some remaining due to rounding - vest one more time after more time passes
			System::set_block_number(vesting_duration * 2);
			assert_ok!(AcurastTokenClaim::vest(RuntimeOrigin::signed(account.clone()), None, None));
		}
		// Now should be fully vested
		assert_eq!(Balances::balance(&account), claim_amount);
		assert!(AcurastTokenClaim::vesting(&account, &account).is_none());
	});
}

#[test]
fn test_starting_block_never_changes() {
	ExtBuilder.build().execute_with(|| {
		let claim_amount: Balance = 100 * UNIT;
		let initial_funds = claim_amount * 10;
		assert_ok!(<Balances as Mutate<_>>::mint_into(
			&<Test as Config>::Funder::get(),
			initial_funds
		));
		let (_, account) = generate_pair_account("User");
		let (signer, _) = generate_pair_account("Bob");
		let signature = generate_signature(&signer, &account, claim_amount);
		let proof = ClaimProof::new(claim_amount, signature);

		let claim_block = System::block_number();
		assert_ok!(AcurastTokenClaim::claim(
			RuntimeOrigin::signed(account.clone()),
			proof,
			account.clone()
		));

		let vesting_duration = <Test as Config>::VestingDuration::get();

		// Vest multiple times and verify starting_block never changes
		for i in 1..=3 {
			let vest_block = (vesting_duration / 4 * i) as u64;
			System::set_block_number(vest_block);

			if let Some(vesting_before) = AcurastTokenClaim::vesting(&account, &account) {
				assert_eq!(
					vesting_before.starting_block, claim_block,
					"starting_block should never change"
				);

				assert_ok!(AcurastTokenClaim::vest(
					RuntimeOrigin::signed(account.clone()),
					None,
					None
				));

				if let Some(vesting_after) = AcurastTokenClaim::vesting(&account, &account) {
					assert_eq!(
						vesting_after.starting_block, claim_block,
						"starting_block should remain unchanged after vest"
					);
					assert_eq!(
						vesting_after.latest_vest, vest_block,
						"latest_vest should be updated to current block"
					);
				}
			}
		}
	});
}

// ============================================================================
// Multiple claimers to same destination tests
// ============================================================================

#[test]
fn test_multiple_claimers_to_same_destination() {
	ExtBuilder.build().execute_with(|| {
		let claim_amount1: Balance = 100 * UNIT;
		let claim_amount2: Balance = 50 * UNIT;
		let initial_funds = (claim_amount1 + claim_amount2) * 10;
		assert_ok!(<Balances as Mutate<_>>::mint_into(
			&<Test as Config>::Funder::get(),
			initial_funds
		));

		let (_, claimer1) = generate_pair_account("Claimer1");
		let (_, claimer2) = generate_pair_account("Claimer2");
		let (_, destination) = generate_pair_account("Destination");
		let (signer, _) = generate_pair_account("Bob");

		// First claimer claims to destination
		let signature1 = generate_signature(&signer, &claimer1, claim_amount1);
		let proof1 = ClaimProof::new(claim_amount1, signature1);
		assert_ok!(AcurastTokenClaim::claim(
			RuntimeOrigin::signed(claimer1.clone()),
			proof1,
			destination.clone()
		));

		// Check vesting exists for (destination, claimer1)
		let vesting1 = AcurastTokenClaim::vesting(&destination, &claimer1);
		assert!(vesting1.is_some());
		assert_eq!(vesting1.unwrap().remaining, claim_amount1);

		// Second claimer claims to same destination
		let signature2 = generate_signature(&signer, &claimer2, claim_amount2);
		let proof2 = ClaimProof::new(claim_amount2, signature2);
		assert_ok!(AcurastTokenClaim::claim(
			RuntimeOrigin::signed(claimer2.clone()),
			proof2,
			destination.clone()
		));

		// Check both vesting schedules exist
		let vesting1 = AcurastTokenClaim::vesting(&destination, &claimer1);
		let vesting2 = AcurastTokenClaim::vesting(&destination, &claimer2);
		assert!(vesting1.is_some());
		assert!(vesting2.is_some());
		assert_eq!(vesting1.unwrap().remaining, claim_amount1);
		assert_eq!(vesting2.unwrap().remaining, claim_amount2);
	});
}

#[test]
fn test_vest_specific_claimer() {
	ExtBuilder.build().execute_with(|| {
		let claim_amount1: Balance = 100 * UNIT;
		let claim_amount2: Balance = 50 * UNIT;
		let initial_funds = (claim_amount1 + claim_amount2) * 10;
		assert_ok!(<Balances as Mutate<_>>::mint_into(
			&<Test as Config>::Funder::get(),
			initial_funds
		));

		let (_, claimer1) = generate_pair_account("Claimer1");
		let (_, claimer2) = generate_pair_account("Claimer2");
		let (_, destination) = generate_pair_account("Destination");
		let (signer, _) = generate_pair_account("Bob");

		// Create two vesting schedules to same destination
		let signature1 = generate_signature(&signer, &claimer1, claim_amount1);
		let proof1 = ClaimProof::new(claim_amount1, signature1);
		assert_ok!(AcurastTokenClaim::claim(
			RuntimeOrigin::signed(claimer1.clone()),
			proof1,
			destination.clone()
		));

		let signature2 = generate_signature(&signer, &claimer2, claim_amount2);
		let proof2 = ClaimProof::new(claim_amount2, signature2);
		assert_ok!(AcurastTokenClaim::claim(
			RuntimeOrigin::signed(claimer2.clone()),
			proof2,
			destination.clone()
		));

		// Move forward to fully vest
		let vesting_duration = <Test as Config>::VestingDuration::get() as u64;
		System::set_block_number(vesting_duration * 2);

		// Vest only claimer2's schedule
		assert_ok!(AcurastTokenClaim::vest(
			RuntimeOrigin::signed(destination.clone()),
			None,                   // uses origin as destination
			Some(claimer2.clone())  // specific claimer
		));

		// Check Vested event with claimer2
		assert!(events().iter().any(|e| matches!(
			e,
			RuntimeEvent::AcurastTokenClaim(crate::Event::Vested {
				destination: d,
				claimer: c,
				remaining: 0
			}) if d == &destination && c == &claimer2
		)));

		// claimer2's schedule should be removed, claimer1's should still exist
		assert!(AcurastTokenClaim::vesting(&destination, &claimer2).is_none());
		assert!(AcurastTokenClaim::vesting(&destination, &claimer1).is_some());

		// Balance should only include claim_amount2
		assert_eq!(Balances::balance(&destination), claim_amount2);

		// Vest claimer1's schedule
		assert_ok!(AcurastTokenClaim::vest(
			RuntimeOrigin::signed(destination.clone()),
			None,
			Some(claimer1.clone())
		));

		// Now both should be removed and balance should be total
		assert!(AcurastTokenClaim::vesting(&destination, &claimer1).is_none());
		assert_eq!(Balances::balance(&destination), claim_amount1 + claim_amount2);
	});
}

#[test]
fn test_vest_wrong_claimer_fails() {
	ExtBuilder.build().execute_with(|| {
		let claim_amount: Balance = 100 * UNIT;
		let initial_funds = claim_amount * 10;
		assert_ok!(<Balances as Mutate<_>>::mint_into(
			&<Test as Config>::Funder::get(),
			initial_funds
		));

		let (_, account) = generate_pair_account("User");
		let (_, wrong_claimer) = generate_pair_account("WrongClaimer");
		let (signer, _) = generate_pair_account("Bob");
		let signature = generate_signature(&signer, &account, claim_amount);
		let proof = ClaimProof::new(claim_amount, signature);

		// Claim to self
		assert_ok!(AcurastTokenClaim::claim(
			RuntimeOrigin::signed(account.clone()),
			proof,
			account.clone()
		));

		// Move forward
		let vesting_duration = <Test as Config>::VestingDuration::get() as u64;
		System::set_block_number(vesting_duration * 2);

		// Try to vest with wrong claimer
		assert_err!(
			AcurastTokenClaim::vest(
				RuntimeOrigin::signed(account.clone()),
				None,
				Some(wrong_claimer) // non-existent claimer for this destination
			),
			Error::<Test>::NotVesting
		);
	});
}

#[test]
fn test_multiple_claims_different_destinations() {
	ExtBuilder.build().execute_with(|| {
		let claim_amount: Balance = 100 * UNIT;
		let initial_funds = claim_amount * 10;
		assert_ok!(<Balances as Mutate<_>>::mint_into(
			&<Test as Config>::Funder::get(),
			initial_funds
		));

		let (_, claimer1) = generate_pair_account("Claimer1");
		let (_, claimer2) = generate_pair_account("Claimer2");
		let (_, destination1) = generate_pair_account("Destination1");
		let (_, destination2) = generate_pair_account("Destination2");
		let (signer, _) = generate_pair_account("Bob");

		// Both claimers claim to different destinations
		let signature1 = generate_signature(&signer, &claimer1, claim_amount);
		let proof1 = ClaimProof::new(claim_amount, signature1);
		assert_ok!(AcurastTokenClaim::claim(
			RuntimeOrigin::signed(claimer1.clone()),
			proof1,
			destination1.clone()
		));

		let signature2 = generate_signature(&signer, &claimer2, claim_amount);
		let proof2 = ClaimProof::new(claim_amount, signature2);
		assert_ok!(AcurastTokenClaim::claim(
			RuntimeOrigin::signed(claimer2.clone()),
			proof2,
			destination2.clone()
		));

		// Both should work
		assert!(AcurastTokenClaim::vesting(&destination1, &claimer1).is_some());
		assert!(AcurastTokenClaim::vesting(&destination2, &claimer2).is_some());
	});
}
