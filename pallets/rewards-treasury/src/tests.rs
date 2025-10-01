#![cfg(test)]

use frame_support::{
	assert_ok,
	traits::{tokens::fungible::Mutate, OnFinalize, OnInitialize},
	weights::Weight,
};
use sp_core::H256;
use sp_runtime::traits::AccountIdConversion;

use crate::{mock::*, stub::*};

#[test]
fn test_single_vest_no_rewards() {
	ExtBuilder::default().build().execute_with(|| {
		let account: AccountId = RTPalletId::get().into_account_truncating();
		assert_eq!(System::block_number(), 1);

		assert_ok!(Balances::mint_into(&alice_account_id(), 10 * UNIT));
		assert_ok!(Balances::mint_into(&bob_account_id(), 20 * UNIT));
		assert_eq!(Balances::free_balance(alice_account_id()), 10 * UNIT);
		assert_eq!(Balances::free_balance(bob_account_id()), 20 * UNIT);
		assert_ok!(Balances::transfer_allow_death(
			RuntimeOrigin::signed(alice_account_id()),
			account.clone(),
			1 * UNIT
		));
		assert_ok!(Balances::transfer_allow_death(
			RuntimeOrigin::signed(alice_account_id()),
			account.clone(),
			3 * UNIT
		));
		assert_eq!(Balances::free_balance(&account), 4 * UNIT);

		assert_eq!(RewardsTreasury::penultimate_balance(), 0);

		// make epoch not yet change
		add_blocks(3);
		assert_eq!(System::block_number(), 4);
		assert_eq!(RewardsTreasury::penultimate_balance(), 0);

		// reset events for less bloat in assertion below
		frame_system::Pallet::<Test>::reset_events();

		// make epoch change
		add_blocks(1);
		assert_eq!(System::block_number(), 5);

		assert_eq!(RewardsTreasury::penultimate_balance(), 4 * UNIT - ExistentialDeposit::get());

		assert_ok!(Balances::transfer_allow_death(
			RuntimeOrigin::signed(alice_account_id()),
			account.clone(),
			3 * UNIT
		));

		// make epoch change
		add_blocks(5);
		assert_eq!(System::block_number(), 10);

		// make epoch change
		add_blocks(5);
		assert_eq!(System::block_number(), 15);

		assert_eq!(
			events(),
			[
				RuntimeEvent::Balances(pallet_balances::Event::Burned {
					who: account.clone(),
					amount: 0
				}),
				RuntimeEvent::RewardsTreasury(crate::Event::BurntFromTreasuryAtEndOfEpoch(0)),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: alice_account_id(),
					to: account.clone(),
					amount: 3 * UNIT
				}),
				RuntimeEvent::Balances(pallet_balances::Event::Burned {
					who: account.clone(),
					amount: 4 * UNIT - ExistentialDeposit::get()
				}),
				RuntimeEvent::RewardsTreasury(crate::Event::BurntFromTreasuryAtEndOfEpoch(
					4 * UNIT - ExistentialDeposit::get()
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Burned {
					who: account.clone(),
					amount: 3 * UNIT
				}),
				RuntimeEvent::RewardsTreasury(crate::Event::BurntFromTreasuryAtEndOfEpoch(
					3 * UNIT
				)),
			]
		);
	});
}

fn next_block() -> Weight {
	let number = frame_system::Pallet::<Test>::block_number();
	RewardsTreasury::on_finalize(number);
	frame_system::Pallet::<Test>::finalize();

	let next_number = number + 1;
	let hash = H256::repeat_byte(next_number as u8);

	frame_system::Pallet::<Test>::initialize(&next_number, &hash, &Default::default());
	RewardsTreasury::on_initialize(next_number)
}

fn add_blocks(blocks: usize) {
	for _ in 0..blocks {
		next_block();
	}
}
