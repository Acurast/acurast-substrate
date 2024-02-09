#![cfg(test)]

use crate::chain::substrate::{MMRProofItems, ProofLeaf, SubstrateProof};
use crate::instances::AlephZeroInstance;
use crate::stub::*;
use crate::types::*;
use crate::{
    mock::*,
    types::{ActivityWindow, StateTransmitterUpdate},
};
use frame_support::assert_ok;
use hex_literal::hex;
use sp_core::H256;
use sp_runtime::{bounded_vec, AccountId32};
use std::marker::PhantomData;

#[test]
fn test_send_noop_message() {
    let mut test = new_test_ext();

    test.execute_with(|| {
        let seq_id_before = 0;
        <crate::MessageSequenceId::<Test, AlephZeroInstance>>::set(seq_id_before);

        let actions = vec![
            StateTransmitterUpdate::Add(
                alice_account_id(),
                ActivityWindow {
                    start_block: 10,
                    end_block: 20,
                },
            ),
            StateTransmitterUpdate::Add(
                bob_account_id(),
                ActivityWindow {
                    start_block: 10,
                    end_block: 50,
                },
            ),
        ];

        assert_ok!(AlephZeroHyperdrive::update_state_transmitters(
            RuntimeOrigin::root().into(),
            StateTransmitterUpdates::<Test>::try_from(actions).unwrap()
        ));
        System::set_block_number(10);

        let snapshot_root_1 = H256(hex!(
            "2aca22363e32e9b3327d6033b769bcf552bfc12fc0aeba82ff4193ad2fc979cc"
        ));
        assert_ok!(
            AlephZeroHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                snapshot_root_1
            )
        );
        assert_ok!(
            AlephZeroHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(bob_account_id()),
                1,
                snapshot_root_1
            )
        );

        assert_eq!(AlephZeroHyperdrive::validate_state_merkle_root(1, snapshot_root_1), true);

        let proof: MMRProofItems = bounded_vec![
        ];
        let leaves: Vec<ProofLeaf> = bounded_vec![
                ProofLeaf {
                    leaf_index: 0,
                    data: hex!("0100000000000000d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d01000404").to_vec()
                }
        ];

        let proof = SubstrateProof::<AcurastAccountId, AccountId32> {
            mmr_size: 1,
            proof,
            leaves,
            marker: PhantomData::default()
        };

        assert_ok!(
            AlephZeroHyperdrive::submit_message(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                proof
            )
        );
    });
}

#[test]
fn test_send_noop_message2() {
    let mut test = new_test_ext();

    test.execute_with(|| {
        let seq_id_before = 10;
        <crate::MessageSequenceId::<Test, AlephZeroInstance>>::set(seq_id_before);

        let actions = vec![
            StateTransmitterUpdate::Add(
                alice_account_id(),
                ActivityWindow {
                    start_block: 10,
                    end_block: 20,
                },
            ),
            StateTransmitterUpdate::Add(
                bob_account_id(),
                ActivityWindow {
                    start_block: 10,
                    end_block: 50,
                },
            ),
        ];

        assert_ok!(AlephZeroHyperdrive::update_state_transmitters(
            RuntimeOrigin::root().into(),
            StateTransmitterUpdates::<Test>::try_from(actions).unwrap()
        ));
        System::set_block_number(10);

        let snapshot_root_1 = H256(hex!(
            "54291e3c7c150d308ab37d46ba6ee933b884397a45b522ad24c829fc77f5d4a2"
        ));
        assert_ok!(
            AlephZeroHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                snapshot_root_1
            )
        );
        assert_ok!(
            AlephZeroHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(bob_account_id()),
                1,
                snapshot_root_1
            )
        );

        assert_eq!(AlephZeroHyperdrive::validate_state_merkle_root(1, snapshot_root_1), true);

        let proof: MMRProofItems = bounded_vec![
            H256(hex!("b4404ad3def37f4a9bd77f585c2cc54bd9908c876b742e3740a08f237dd3b0a4")),
            H256(hex!("5ea45db0cf6643d53b5dbaa5e30f6f5c062ae79deb0cd62c1545169cea9e619f"))
        ];
        let leaves: Vec<ProofLeaf> = bounded_vec![
            ProofLeaf {
                leaf_index: 18,
                data: hex!("0b00000000000000d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d01000404").to_vec()
            }
        ];

        let proof = SubstrateProof::<AcurastAccountId, AccountId32> {
            mmr_size: 19,
            proof,
            leaves,
            marker: PhantomData::default()
        };

        assert_ok!(
            AlephZeroHyperdrive::submit_message(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                proof
            )
        );
    });
}

#[test]
fn test_send_noop_message3() {
    let mut test = new_test_ext();

    test.execute_with(|| {
        let seq_id_before = 29;
        <crate::MessageSequenceId::<Test, AlephZeroInstance>>::set(seq_id_before);

        let actions = vec![
            StateTransmitterUpdate::Add(
                alice_account_id(),
                ActivityWindow {
                    start_block: 10,
                    end_block: 20,
                },
            ),
            StateTransmitterUpdate::Add(
                bob_account_id(),
                ActivityWindow {
                    start_block: 10,
                    end_block: 50,
                },
            ),
        ];

        assert_ok!(AlephZeroHyperdrive::update_state_transmitters(
            RuntimeOrigin::root().into(),
            StateTransmitterUpdates::<Test>::try_from(actions).unwrap()
        ));
        System::set_block_number(10);

        let snapshot_root_1 = H256(hex!(
            "e8df79ce243d822375449fd1310856176e0ac84db0f8ba35e06cccd831057305"
        ));
        assert_ok!(
            AlephZeroHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                snapshot_root_1
            )
        );
        assert_ok!(
            AlephZeroHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(bob_account_id()),
                1,
                snapshot_root_1
            )
        );

        assert_eq!(AlephZeroHyperdrive::validate_state_merkle_root(1, snapshot_root_1), true);

        let proof: MMRProofItems = bounded_vec![
            H256(hex!("a7ab94f788d835316ee20779fa62b27b4897cbeb08213f4302805d86f1412064")),
            H256(hex!("f989478c7bb910fb6be34a94e8f961c13a47b1794268c5bcfea29592e25a2166")),
            H256(hex!("f1847fb42b6a5f3ffa591d68250220c3238a6c6f24ac653296e5edc9584ab959")),
            H256(hex!("125b90ef2fe7b6fa986de87faa2dd3f671fb0bd13f2d2b54292cef6deec69d15")),
        ];
        let leaves: Vec<ProofLeaf> = bounded_vec![
                ProofLeaf {
                    leaf_index: 11,
                    data: hex!("1e00000000000000d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d01000404").to_vec()
                }
        ];

        let proof = SubstrateProof::<AcurastAccountId, AccountId32> {
            mmr_size: 19,
            proof,
            leaves,
            marker: PhantomData::default()
        };

        assert_ok!(
            AlephZeroHyperdrive::submit_message(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                proof
            )
        );
    });
}
