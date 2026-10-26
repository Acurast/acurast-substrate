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

fn setup_expired_transfer(
	initial_block: u64,
	initial_balance: u128,
	amount: u128,
	fee: u128,
) -> (u64, sp_core::H256) {
	let _ = Balances::force_set_balance(RuntimeOrigin::root(), alice_account_id(), initial_balance);
	let _ = Balances::force_set_balance(RuntimeOrigin::root(), bob_account_id(), initial_balance);
	let _ = Balances::force_set_balance(RuntimeOrigin::root(), ethereum_vault(), initial_balance);
	let _ =
		Balances::force_set_balance(RuntimeOrigin::root(), ethereum_fee_vault(), initial_balance);
	EthereumContract::<Test>::set(Some(ethereum_token_contract()));
	Enabled::<Test>::set(Some(true));

	System::set_block_number(initial_block);
	assert_ok!(AcurastHyperdriveToken::transfer_native(
		RuntimeOrigin::signed(alice_account_id()),
		ethereum_dest(),
		amount,
		fee,
	));
	let nonce = AcurastHyperdriveToken::next_transfer_nonce(ProxyChain::Ethereum) - 1;
	let id = pallet_acurast_hyperdrive_ibc::OutgoingMessages::<Test>::iter_keys()
		.next()
		.unwrap();

	// let the message expire
	System::set_block_number(initial_block + 16);
	(nonce, id)
}

#[test]
fn test_remove_message_restricted_to_sender_or_update_origin() {
	try_init_simple(); // ignore error if already initialized

	let initial_block = 10;
	let initial_balance = 1000 * UNIT;
	let amount_to_transfer = UNIT;
	let fee_amount = UNIT / 10;
	let retry_fee_amount = 2 * fee_amount;

	new_test_ext().execute_with(|| {
		let (nonce, id) =
			setup_expired_transfer(initial_block, initial_balance, amount_to_transfer, fee_amount);

		// neither an unrelated account nor the transfer source (who is neither sender nor payer of the message)
		// can remove the message directly on hyperdrive-ibc
		for who in [bob_account_id(), alice_account_id()] {
			assert_err!(
				AcurastHyperdriveIbc::remove_message(RuntimeOrigin::signed(who), id),
				pallet_acurast_hyperdrive_ibc::Error::<Test>::NotAllowedToRemoveMessage,
			);
		}

		// retry still refunds the previous fee to the source
		assert_ok!(AcurastHyperdriveToken::retry_transfer_native(
			RuntimeOrigin::signed(alice_account_id()),
			ProxyChain::Ethereum,
			nonce,
			retry_fee_amount,
		));
		assert_eq!(
			Balances::free_balance(alice_account_id()),
			initial_balance - amount_to_transfer - retry_fee_amount
		);
		assert_eq!(Balances::free_balance(ethereum_fee_vault()), initial_balance);
		assert_eq!(Balances::reserved_balance(ethereum_fee_vault()), retry_fee_amount);

		// the privileged origin can still remove expired messages
		System::set_block_number(initial_block + 32);
		assert_ok!(AcurastHyperdriveIbc::remove_message(RuntimeOrigin::root(), id));
		assert!(pallet_acurast_hyperdrive_ibc::OutgoingMessages::<Test>::get(id).is_none());
		assert_eq!(Balances::reserved_balance(ethereum_fee_vault()), 0);
	});
}

#[test]
fn test_remove_transfer_message_refunds_fee_to_source() {
	try_init_simple(); // ignore error if already initialized

	let initial_block = 10;
	let initial_balance = 1000 * UNIT;
	let amount_to_transfer = UNIT;
	let fee_amount = UNIT / 10;

	new_test_ext().execute_with(|| {
		System::set_block_number(initial_block);
		let (nonce, id) =
			setup_expired_transfer(initial_block, initial_balance, amount_to_transfer, fee_amount);

		// only the source of the transfer can remove its message
		assert_err!(
			AcurastHyperdriveToken::remove_transfer_message(
				RuntimeOrigin::signed(bob_account_id()),
				ProxyChain::Ethereum,
				nonce,
			),
			Error::<Test>::InvalidSender,
		);

		assert_ok!(AcurastHyperdriveToken::remove_transfer_message(
			RuntimeOrigin::signed(alice_account_id()),
			ProxyChain::Ethereum,
			nonce,
		));

		assert!(pallet_acurast_hyperdrive_ibc::OutgoingMessages::<Test>::get(id).is_none());
		assert_eq!(
			Balances::free_balance(alice_account_id()),
			initial_balance - amount_to_transfer
		);
		assert_eq!(Balances::free_balance(ethereum_fee_vault()), initial_balance);
		assert_eq!(Balances::reserved_balance(ethereum_fee_vault()), 0);
		// amount stays locked and transfer remains retryable
		assert_eq!(Balances::free_balance(ethereum_vault()), initial_balance + amount_to_transfer);
		assert!(OutgoingTransfers::<Test>::get(ProxyChain::Ethereum, nonce).is_some());
		System::assert_has_event(
			Event::TransferMessageRemoved {
				proxy: ProxyChain::Ethereum,
				transfer_nonce: nonce,
				source: alice_account_id(),
				fee: fee_amount,
			}
			.into(),
		);

		// a second removal fails since the message is gone
		assert_err!(
			AcurastHyperdriveToken::remove_transfer_message(
				RuntimeOrigin::signed(alice_account_id()),
				ProxyChain::Ethereum,
				nonce,
			),
			pallet_acurast_hyperdrive_ibc::Error::<Test>::MessageNotFound,
		);
	});
}

#[test]
fn test_remove_transfer_message_pending_fails() {
	try_init_simple(); // ignore error if already initialized

	new_test_ext().execute_with(|| {
		let (nonce, _) = setup_expired_transfer(10, 1000 * UNIT, UNIT, UNIT / 10);
		// back within ttl
		System::set_block_number(25);
		assert_err!(
			AcurastHyperdriveToken::remove_transfer_message(
				RuntimeOrigin::signed(alice_account_id()),
				ProxyChain::Ethereum,
				nonce,
			),
			pallet_acurast_hyperdrive_ibc::Error::<Test>::CannotRemovePendingMessage,
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
fn test_refund_transfers_removes_expired_messages() {
	let initial_balance = 1000 * UNIT;
	let amount = UNIT;
	let fee = UNIT / 10;

	new_test_ext().execute_with(|| {
		let nonces = setup_outgoing_transfers(initial_balance, amount, fee, 2);

		// both messages expire, the second transfer is retried and therefore has a pending message again
		System::set_block_number(26);
		assert_ok!(AcurastHyperdriveToken::retry_transfer_native(
			RuntimeOrigin::signed(alice_account_id()),
			ProxyChain::Ethereum,
			nonces[1],
			fee,
		));
		assert_eq!(
			Balances::free_balance(alice_account_id()),
			initial_balance - 2 * amount - 2 * fee
		);
		System::reset_events();

		assert_ok!(AcurastHyperdriveToken::refund_transfers(
			RuntimeOrigin::root(),
			ProxyChain::Ethereum,
			nonces.clone().try_into().unwrap(),
		));

		// amounts of both transfers and the fee of the expired message are refunded
		assert_eq!(Balances::free_balance(alice_account_id()), initial_balance - fee);
		assert_eq!(Balances::free_balance(ethereum_fee_vault()), initial_balance);
		// the pending message stays with its fee held
		assert_eq!(Balances::reserved_balance(ethereum_fee_vault()), fee);
		assert_eq!(pallet_acurast_hyperdrive_ibc::OutgoingMessages::<Test>::iter().count(), 1);
		System::assert_has_event(
			Event::TransferMessageRemoved {
				proxy: ProxyChain::Ethereum,
				transfer_nonce: nonces[0],
				source: alice_account_id(),
				fee,
			}
			.into(),
		);
		for nonce in nonces {
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
