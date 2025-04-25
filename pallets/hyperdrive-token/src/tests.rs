#![cfg(test)]

use frame_support::{assert_err, assert_ok, error::BadOrigin};
use hex_literal::hex;
use pallet_acurast::{AccountId20, ProxyChain};
use sp_runtime::AccountId32;
use sp_tracing::try_init_simple;

use crate::{
	mock::*, stub::*, Enabled, Error, EthereumContract, Event, NextTransferNonce,
	OutgoingTransfers, SolanaContract,
};

#[test]
fn test_transfer_native_success() {
	let _ = try_init_simple(); // ignore error if already initialized

	let initial_block = 10;
	let mut test = new_test_ext();
	let initial_balance = 1000 * UNIT;
	let amount_to_transfer = 1 * UNIT;
	let fee_amount = 2 * MILLIUNIT;
	let retry_fee_amount = 3 * MILLIUNIT;
	let initial_nonce = 74;

	test.execute_with(|| {
		// Arrange: initial balances and configuration
		let _ =
			Balances::force_set_balance(RuntimeOrigin::root(), alice_account_id(), initial_balance);
		let _ =
			Balances::force_set_balance(RuntimeOrigin::root(), ethereum_vault(), initial_balance);
		let _ = Balances::force_set_balance(
			RuntimeOrigin::root(),
			ethereum_fee_vault(),
			initial_balance,
		);

		assert_eq!(Balances::free_balance(&alice_account_id()), initial_balance);
		assert_eq!(Balances::free_balance(&ethereum_vault()), initial_balance);
		assert_eq!(Balances::free_balance(&ethereum_fee_vault()), initial_balance);

		NextTransferNonce::<Test>::set(ProxyChain::Ethereum, initial_nonce);
		EthereumContract::<Test>::set(Some(ethereum_token_contract()));
		Enabled::<Test>::set(Some(true));

		// Act
		System::set_block_number(initial_block);
		System::reset_events(); // Clear events before action
		assert_ok!(AcurastHyperdriveToken::transfer_native(
			RuntimeOrigin::signed(alice_account_id()),
			ethereum_dest(),
			amount_to_transfer.into(),
			fee_amount.into(),
		));

		// Assert: state changes
		assert_eq!(
			AcurastHyperdriveToken::next_transfer_nonce(ProxyChain::Ethereum),
			initial_nonce + 1
		);
		assert_eq!(
			Balances::free_balance(&alice_account_id()),
			initial_balance - amount_to_transfer - fee_amount
		);
		assert_eq!(Balances::free_balance(ethereum_vault()), initial_balance + amount_to_transfer);
		assert_eq!(Balances::free_balance(ethereum_fee_vault()), initial_balance);
		assert_eq!(Balances::reserved_balance(ethereum_fee_vault()), fee_amount);

		// Assert: Outgoing transfer details stored
		{
			let (source, dest, amount) =
				OutgoingTransfers::<Test>::get(ProxyChain::Ethereum, initial_nonce).unwrap();
			assert_eq!(source, alice_account_id()); // source
			assert_eq!(dest, ethereum_dest()); // dest
			assert_eq!(amount, amount_to_transfer.into()); // amount
		}

		// Assert: Event emitted
		System::assert_has_event(
			Event::TransferToProxy {
				source: alice_account_id(),
				dest: ethereum_dest(),
				amount: amount_to_transfer.into(),
			}
			.into(),
		);

		// Act (retry)
		System::set_block_number(initial_block + 15);
		assert_err!(
			AcurastHyperdriveToken::retry_transfer_native(
				RuntimeOrigin::signed(alice_account_id()), // Must be called by the original sender
				ProxyChain::Ethereum,
				initial_nonce,
				retry_fee_amount.into(),
			),
			Error::<Test>::PalletHyperdriveIBC(
				pallet_acurast_hyperdrive_ibc::Error::<Test>::MessageWithSameNoncePending
			)
		);

		System::set_block_number(initial_block + 16);
		System::reset_events(); // Clear events before action
		assert_ok!(AcurastHyperdriveToken::retry_transfer_native(
			RuntimeOrigin::signed(alice_account_id()), // Must be called by the original sender
			ProxyChain::Ethereum,
			initial_nonce,
			retry_fee_amount.into(),
		));

		// Assert: State changes
		// Nonce should NOT change on retry
		assert_eq!(
			AcurastHyperdriveToken::next_transfer_nonce(ProxyChain::Ethereum),
			initial_nonce + 1
		);
		// Source balance decreases ONLY by the retry fee
		assert_eq!(
			Balances::free_balance(&alice_account_id()),
			initial_balance - amount_to_transfer - fee_amount - retry_fee_amount
		);
		// Vault balance for the *amount* should NOT change
		assert_eq!(Balances::free_balance(ethereum_vault()), initial_balance + amount_to_transfer);
		// Fee vault locked balance increases by the retry fee
		assert_eq!(Balances::reserved_balance(ethereum_fee_vault()), fee_amount + retry_fee_amount);

		// Outgoing transfer record remains unchanged
		{
			let (source, dest, amount) =
				OutgoingTransfers::<Test>::get(ProxyChain::Ethereum, initial_nonce).unwrap();
			assert_eq!(source, alice_account_id()); // source
			assert_eq!(dest, ethereum_dest()); // dest
			assert_eq!(amount, amount_to_transfer.into()); // amount
		}

		// Assert: Event emitted for the retry attempt (the pallet emits TransferToProxy again)
		System::assert_has_event(
			Event::TransferToProxy {
				source: alice_account_id(),
				dest: ethereum_dest(),
				amount: amount_to_transfer, // Event shows original amount
			}
			.into(),
		);
	});
}

#[test]
fn test_update_ethereum_contract_success() {
	let mut test = new_test_ext();
	let new_contract = AccountId20(hex!("1111111111111111111111111111111111111111"));

	test.execute_with(|| {
		// Arrange: Ensure contract is initially None or different
		EthereumContract::<Test>::kill();
		assert!(AcurastHyperdriveToken::ethereum_contract().is_none());
		System::reset_events();

		// Act: Update contract as root
		assert_ok!(AcurastHyperdriveToken::update_ethereum_contract(
			RuntimeOrigin::root(),
			new_contract.clone()
		));

		// Assert: Storage updated
		assert_eq!(AcurastHyperdriveToken::ethereum_contract(), Some(new_contract.clone()));

		// Assert: Event emitted
		System::assert_last_event(Event::EthereumContractUpdated { contract: new_contract }.into());
	});
}

#[test]
fn test_update_ethereum_contract_fail_bad_origin() {
	let mut test = new_test_ext();
	let new_contract = AccountId20(hex!("1111111111111111111111111111111111111111"));
	let non_root_caller = alice_account_id();

	test.execute_with(|| {
		// Act & Assert: Try to update as non-root
		assert_err!(
			AcurastHyperdriveToken::update_ethereum_contract(
				RuntimeOrigin::signed(non_root_caller),
				new_contract.clone()
			),
			BadOrigin // Expecting a BadOrigin error for non-root calls
		);

		// Assert: Storage not updated
		assert!(AcurastHyperdriveToken::ethereum_contract().is_none());
	});
}

#[test]
fn test_update_solana_contract_success() {
	let mut test = new_test_ext();
	let new_contract = AccountId32::new([5u8; 32]); // Example Solana address (as AccountId32)

	test.execute_with(|| {
		// Arrange: Ensure contract is initially None or different
		SolanaContract::<Test>::kill();
		assert!(AcurastHyperdriveToken::solana_contract().is_none());
		System::reset_events();

		// Act: Update contract as root
		assert_ok!(AcurastHyperdriveToken::update_solana_contract(
			RuntimeOrigin::root(),
			new_contract.clone()
		));

		// Assert: Storage updated
		assert_eq!(AcurastHyperdriveToken::solana_contract(), Some(new_contract.clone()));

		// Assert: Event emitted
		System::assert_last_event(Event::SolanaContractUpdated { contract: new_contract }.into());
	});
}

#[test]
fn test_update_solana_contract_fail_bad_origin() {
	let mut test = new_test_ext();
	let new_contract = AccountId32::new([5u8; 32]);
	let non_root_caller = alice_account_id();

	test.execute_with(|| {
		// Act & Assert: Try to update as non-root
		assert_err!(
			AcurastHyperdriveToken::update_solana_contract(
				RuntimeOrigin::signed(non_root_caller),
				new_contract.clone()
			),
			BadOrigin // Expecting a BadOrigin error for non-root calls
		);

		// Assert: Storage not updated
		assert!(AcurastHyperdriveToken::solana_contract().is_none());
	});
}
