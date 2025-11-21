use frame_support::{
	assert_err, assert_ok,
	traits::fungible::{Inspect, Mutate},
};

use crate::{mock::*, ClaimProof, Config, Error};

#[test]
fn test_claim() {
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
		assert_eq!(
			events().last(),
			Some(&RuntimeEvent::AcurastTokenClaim(crate::Event::Claimed {
				account: account.clone(),
				amount: claim_amount
			}))
		);

		assert_eq!(
			Balances::balance(&<Test as Config>::Funder::get()),
			initial_funds - claim_amount
		);
		assert_eq!(Balances::balance(&account), claim_amount);
		let vesting = Vesting::vesting(account);
		assert_ne!(vesting, None);
		let vesting = vesting.unwrap();
		assert!(!vesting.is_empty());
	});
}

#[test]
fn test_claim_failure_1() {
	ExtBuilder.build().execute_with(|| {
		let claim_amount: Balance = 100 * UNIT;
		let initial_funds = claim_amount * 10;
		assert_ok!(<Balances as Mutate<_>>::mint_into(
			&<Test as Config>::Funder::get(),
			initial_funds
		));
		let (_, account) = generate_pair_account("User");
		let (signer, _) = generate_pair_account("Charlie");
		let signature = generate_signature(&signer, &account, claim_amount);
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
fn test_claim_failure_2() {
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
			proof.clone(),
			account.clone()
		));
		assert_eq!(
			events().last(),
			Some(&RuntimeEvent::AcurastTokenClaim(crate::Event::Claimed {
				account: account.clone(),
				amount: claim_amount
			}))
		);

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
