#![cfg(test)]

use frame_support::{assert_err, assert_ok, error::BadOrigin};
use hex_literal::hex;
use pallet_acurast_hyperdrive_ibc::Instance1;
use sp_runtime::{bounded_vec, traits::Keccak256, AccountId32};

use crate::{mock::*, stub::*, Error};

#[test]
fn test_transfer_native() {
	let mut test = new_test_ext();

	test.execute_with(|| {
		// // pretend given message seq_id was just before test message 75 arrives
		// let next_transfer_nonce = 74;
		// <crate::NextTransferNonce::<Test, Instance1>>::set(next_transfer_nonce);

		// // set the ethereum contract to some dummy value
		// <crate::EthereumContract::<Test, Instance1>>::set(hex!("7F44aD0fD6c15CfBA6f417C33924c8cF0C751d23"));

		// let tezos_contract = ProxyAddress::try_from(hex!("050a000000160199651cbe1a155a5c8e5af7d6ea5c3f48eebb8c9c00").to_vec()).unwrap();
		// assert_ok!(AcurastHyperdriveToken::transfer_native(
		//     RuntimeOrigin::root().into(),
		//     // TODO
		// ));

		// assert_eq!(AcurastHyperdriveToken::next_transfer_nonce(), next_transfer_nonce + 1);
	});
}
