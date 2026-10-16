use frame_support::{assert_err, assert_ok, sp_runtime::traits::BadOrigin};
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
	try_init_simple(); // ignore error if already initialized

	let initial_block = 10;
	let mut test = new_test_ext();
	let initial_balance = 1000 * UNIT;
	let amount_to_transfer = UNIT;
	let fee_amount = UNIT / 10;
	let retry_fee_amount = 2 * fee_amount;
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

		assert_eq!(Balances::free_balance(alice_account_id()), initial_balance);
		assert_eq!(Balances::free_balance(ethereum_vault()), initial_balance);
		assert_eq!(Balances::free_balance(ethereum_fee_vault()), initial_balance);

		NextTransferNonce::<Test>::set(ProxyChain::Ethereum, initial_nonce);
		EthereumContract::<Test>::set(Some(ethereum_token_contract()));
		Enabled::<Test>::set(Some(true));

		// Act
		System::set_block_number(initial_block);
		System::reset_events(); // Clear events before action
		assert_ok!(AcurastHyperdriveToken::transfer_native(
			RuntimeOrigin::signed(alice_account_id()),
			ethereum_dest(),
			amount_to_transfer,
			fee_amount,
		));

		// Assert: state changes
		assert_eq!(
			AcurastHyperdriveToken::next_transfer_nonce(ProxyChain::Ethereum),
			initial_nonce + 1
		);
		assert_eq!(
			Balances::free_balance(alice_account_id()),
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
			assert_eq!(amount, amount_to_transfer); // amount
		}

		// Assert: Event emitted
		System::assert_has_event(
			Event::TransferToProxy {
				source: alice_account_id(),
				dest: ethereum_dest(),
				amount: amount_to_transfer,
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
				retry_fee_amount,
			),
			pallet_acurast_hyperdrive_ibc::Error::<Test>::MessageWithSameNoncePending,
		);

		System::set_block_number(initial_block + 16);
		System::reset_events(); // Clear events before action
		assert_ok!(AcurastHyperdriveToken::retry_transfer_native(
			RuntimeOrigin::signed(alice_account_id()), // Must be called by the original sender
			ProxyChain::Ethereum,
			initial_nonce,
			retry_fee_amount,
		));

		// Assert: State changes
		// Nonce should NOT change on retry
		assert_eq!(
			AcurastHyperdriveToken::next_transfer_nonce(ProxyChain::Ethereum),
			initial_nonce + 1
		);
		// Source balance decreases ONLY by the retry fee
		assert_eq!(
			Balances::free_balance(alice_account_id()),
			initial_balance - amount_to_transfer - retry_fee_amount
		);
		// Vault balance for the *amount* should NOT change
		assert_eq!(Balances::free_balance(ethereum_vault()), initial_balance + amount_to_transfer);
		// Fee vault locked balance increases by the retry fee
		assert_eq!(Balances::reserved_balance(ethereum_fee_vault()), retry_fee_amount);

		// Outgoing transfer record remains unchanged
		{
			let (source, dest, amount) =
				OutgoingTransfers::<Test>::get(ProxyChain::Ethereum, initial_nonce).unwrap();
			assert_eq!(source, alice_account_id()); // source
			assert_eq!(dest, ethereum_dest()); // dest
			assert_eq!(amount, amount_to_transfer); // amount
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
			new_contract
		));

		// Assert: Storage updated
		assert_eq!(AcurastHyperdriveToken::ethereum_contract(), Some(new_contract));

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
				new_contract
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

/// Sets up balances and configuration, then makes `count` transfers of `amount` from alice, returning the transfer nonces.
fn setup_outgoing_transfers(
	initial_balance: u128,
	amount: u128,
	fee: u128,
	count: u64,
) -> Vec<u64> {
	let _ = Balances::force_set_balance(RuntimeOrigin::root(), alice_account_id(), initial_balance);
	let _ = Balances::force_set_balance(RuntimeOrigin::root(), ethereum_vault(), initial_balance);
	let _ =
		Balances::force_set_balance(RuntimeOrigin::root(), ethereum_fee_vault(), initial_balance);
	EthereumContract::<Test>::set(Some(ethereum_token_contract()));
	Enabled::<Test>::set(Some(true));
	System::set_block_number(10);

	(0..count)
		.map(|_| {
			let nonce = AcurastHyperdriveToken::next_transfer_nonce(ProxyChain::Ethereum);
			assert_ok!(AcurastHyperdriveToken::transfer_native(
				RuntimeOrigin::signed(alice_account_id()),
				ethereum_dest(),
				amount,
				fee,
			));
			nonce
		})
		.collect()
}

#[test]
fn test_refund_transfers_success() {
	let initial_balance = 1000 * UNIT;
	let amount = UNIT;
	let fee = UNIT / 10;

	new_test_ext().execute_with(|| {
		let nonces = setup_outgoing_transfers(initial_balance, amount, fee, 3);
		let vault_before = Balances::free_balance(ethereum_vault());
		let alice_before = Balances::free_balance(alice_account_id());
		System::reset_events();

		// refund the first and last transfer
		assert_ok!(AcurastHyperdriveToken::refund_transfers(
			RuntimeOrigin::root(),
			ProxyChain::Ethereum,
			vec![nonces[0], nonces[2]].try_into().unwrap(),
		));

		assert_eq!(Balances::free_balance(alice_account_id()), alice_before + 2 * amount);
		assert_eq!(Balances::free_balance(ethereum_vault()), vault_before - 2 * amount);
		assert!(OutgoingTransfers::<Test>::get(ProxyChain::Ethereum, nonces[0]).is_none());
		assert!(OutgoingTransfers::<Test>::get(ProxyChain::Ethereum, nonces[1]).is_some());
		assert!(OutgoingTransfers::<Test>::get(ProxyChain::Ethereum, nonces[2]).is_none());
		for nonce in [nonces[0], nonces[2]] {
			System::assert_has_event(
				Event::TransferRefunded {
					proxy: ProxyChain::Ethereum,
					transfer_nonce: nonce,
					source: alice_account_id(),
					amount,
				}
				.into(),
			);
		}

		// refunded transfers can neither be retried nor refunded again
		System::set_block_number(100);
		assert_err!(
			AcurastHyperdriveToken::retry_transfer_native(
				RuntimeOrigin::signed(alice_account_id()),
				ProxyChain::Ethereum,
				nonces[0],
				fee,
			),
			Error::<Test>::UnknownTransferRetry
		);
		assert_err!(
			AcurastHyperdriveToken::refund_transfers(
				RuntimeOrigin::root(),
				ProxyChain::Ethereum,
				vec![nonces[2]].try_into().unwrap(),
			),
			Error::<Test>::UnknownTransfer
		);
	});
}

#[test]
fn test_refund_transfers_unknown_nonce_reverts_all() {
	let initial_balance = 1000 * UNIT;
	let amount = UNIT;
	let fee = UNIT / 10;

	new_test_ext().execute_with(|| {
		let nonces = setup_outgoing_transfers(initial_balance, amount, fee, 1);
		let alice_before = Balances::free_balance(alice_account_id());

		assert_err!(
			AcurastHyperdriveToken::refund_transfers(
				RuntimeOrigin::root(),
				ProxyChain::Ethereum,
				vec![nonces[0], nonces[0] + 1].try_into().unwrap(),
			),
			Error::<Test>::UnknownTransfer
		);

		assert_eq!(Balances::free_balance(alice_account_id()), alice_before);
		assert!(OutgoingTransfers::<Test>::get(ProxyChain::Ethereum, nonces[0]).is_some());
	});
}

#[test]
fn test_refund_transfers_fail_bad_origin() {
	let initial_balance = 1000 * UNIT;
	let amount = UNIT;
	let fee = UNIT / 10;

	new_test_ext().execute_with(|| {
		let nonces = setup_outgoing_transfers(initial_balance, amount, fee, 1);

		assert_err!(
			AcurastHyperdriveToken::refund_transfers(
				RuntimeOrigin::signed(alice_account_id()),
				ProxyChain::Ethereum,
				nonces.try_into().unwrap(),
			),
			BadOrigin
		);
	});
}
