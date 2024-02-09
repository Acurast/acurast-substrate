use frame_support::assert_ok;
use frame_support::pallet_prelude::*;
use hex_literal::hex;
use mmr_lib::helper;
use sp_core::{
    offchain::{testing::TestOffchainExt, OffchainDbExt, OffchainWorkerExt},
    H256,
};
use sp_runtime::BuildStorage;

use types::Proof;
use utils;

use crate::mmr::NodeOf;
use crate::stub::*;
use crate::{mock::*, *};

pub(crate) fn new_test_ext() -> sp_io::TestExternalities {
    frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap()
        .into()
}

fn register_offchain_ext(ext: &mut sp_io::TestExternalities) {
    let (offchain, _offchain_state) = TestOffchainExt::with_offchain_db(ext.offchain_db());
    ext.register_extension(OffchainDbExt::new(offchain.clone()));
    ext.register_extension(OffchainWorkerExt::new(offchain));
}

fn next_block() -> Weight {
    let number = frame_system::Pallet::<Test>::block_number();
    HyperdriveOutgoing::on_finalize(number);
    frame_system::Pallet::<Test>::finalize();

    let next_number = number + 1;
    frame_system::Pallet::<Test>::reset_events();
    let hash = H256::repeat_byte(next_number as u8);

    frame_system::Pallet::<Test>::initialize(&next_number, &hash, &Default::default());
    HyperdriveOutgoing::on_initialize(next_number)
}

fn add_blocks(blocks: usize) {
    for _ in 0..blocks {
        next_block();
    }
}

fn peaks_from_leaves_count(leaves_count: NodeIndex) -> Vec<NodeIndex> {
    let size = utils::NodesUtils::new(leaves_count).size();
    helper::get_peaks(size)
}

pub(crate) fn hex(s: &str) -> H256 {
    s.parse().unwrap()
}

// type BlockNumber = <Test as frame_system::Config>::BlockNumber;

fn decode_node(v: Vec<u8>) -> NodeOf<Test, ()> {
    let node: NodeOf<Test, ()> = codec::Decode::decode(&mut &v[..]).unwrap();
    node
}

fn send_messages(num: usize) {
    // given
    for id in 0..num {
        assert_ok!(HyperdriveOutgoing::send_message(action(id as u128)));
    }
}

#[test]
fn should_start_empty() {
    let _ = env_logger::try_init();
    new_test_ext().execute_with(|| {
        // given
        assert_eq!(
            crate::RootHash::<Test>::get(),
            "0000000000000000000000000000000000000000000000000000000000000000"
                .parse()
                .unwrap()
        );
        assert_eq!(crate::NumberOfLeaves::<Test>::get(), 0);
        assert_eq!(crate::Nodes::<Test>::get(0), None);

        // when
        let weight = next_block();
        assert_ok!(HyperdriveOutgoing::send_message(action(0)));

        // then
        assert_eq!(crate::NumberOfLeaves::<Test>::get(), 1);
        assert_eq!(
            crate::Nodes::<Test>::get(0),
            Some(hex(
                "cb0c62b3cab675b8b9ca7106c5bbe219048c1af50b1c9ed360443e7df108189d"
            ))
        );
        assert_eq!(
            crate::RootHash::<Test>::get(),
            hex("cb0c62b3cab675b8b9ca7106c5bbe219048c1af50b1c9ed360443e7df108189d")
        );
        assert!(weight != Weight::zero());
    });
}

#[test]
fn should_append_to_mmr_when_send_message_is_called() {
    let _ = env_logger::try_init();
    let mut ext = new_test_ext();

    let root_0 = hex("cb0c62b3cab675b8b9ca7106c5bbe219048c1af50b1c9ed360443e7df108189d");
    let root_1 = hex("bf9f9624420719df2fcbcfd14d0071aa8668c3433f6013bb253f4b4e03587b77");
    let (parent_b1, parent_b2) = ext.execute_with(|| {
        // when
        next_block();
        assert_ok!(HyperdriveOutgoing::send_message(action(0)));
        let parent_b1 = <frame_system::Pallet<Test>>::parent_hash();

        // then
        assert_eq!(crate::NumberOfLeaves::<Test>::get(), 1); // single node that is equal to root
        assert_eq!(
            (
                crate::Nodes::<Test>::get(0),
                crate::Nodes::<Test>::get(1),
                crate::RootHash::<Test>::get(),
            ),
            (
                Some(hex(
                    "cb0c62b3cab675b8b9ca7106c5bbe219048c1af50b1c9ed360443e7df108189d"
                )),
                None,
                root_0,
            )
        );

        // when
        next_block();
        assert_ok!(HyperdriveOutgoing::send_message(action(1)));
        let parent_b2 = <frame_system::Pallet<Test>>::parent_hash();

        // then
        assert_eq!(crate::NumberOfLeaves::<Test>::get(), 2);
        let peaks = peaks_from_leaves_count(2);
        assert_eq!(peaks, vec![2]);
        assert_eq!(
            (
                crate::Nodes::<Test>::get(0),
                crate::Nodes::<Test>::get(1),
                crate::Nodes::<Test>::get(2), // only inner node
                crate::Nodes::<Test>::get(3),
                crate::RootHash::<Test>::get(),
            ),
            (
                None,
                None,
                Some(hex(
                    "bf9f9624420719df2fcbcfd14d0071aa8668c3433f6013bb253f4b4e03587b77"
                )),
                None,
                root_1,
            )
        );

        (parent_b1, parent_b2)
    });
    // make sure the leaves end up in the offchain DB
    ext.persist_offchain_overlay();

    let offchain_db = ext.offchain_db();

    assert_eq!(
        offchain_db
            .get(&HyperdriveOutgoing::node_temp_offchain_key(
                0, parent_b1, root_0
            ))
            .map(decode_node),
        Some(Node::Data(message(0)))
    );

    assert_eq!(
        offchain_db
            .get(&HyperdriveOutgoing::node_temp_offchain_key(
                1, parent_b2, root_1
            ))
            .map(decode_node),
        Some(Node::Data(message(1)))
    );

    assert_eq!(
        offchain_db
            .get(&HyperdriveOutgoing::node_temp_offchain_key(
                2,
                parent_b2,
                hex("bf9f9624420719df2fcbcfd14d0071aa8668c3433f6013bb253f4b4e03587b77")
            ))
            .map(decode_node),
        Some(Node::Hash(hex(
            "bf9f9624420719df2fcbcfd14d0071aa8668c3433f6013bb253f4b4e03587b77",
        )))
    );

    assert_eq!(
        offchain_db.get(&HyperdriveOutgoing::node_temp_offchain_key(
            3,
            parent_b2,
            hex("bf9f9624420719df2fcbcfd14d0071aa8668c3433f6013bb253f4b4e03587b77")
        )),
        None
    );
}

#[test]
fn should_construct_larger_mmr_correctly() {
    let _ = env_logger::try_init();
    new_test_ext().execute_with(|| {
        // when
        send_messages(7);

        // then
        assert_eq!(crate::NumberOfLeaves::<Test>::get(), 7);
        let peaks = peaks_from_leaves_count(7);
        assert_eq!(peaks, vec![6, 9, 10]);
        for i in (0..=10).filter(|p| !peaks.contains(p)) {
            assert!(crate::Nodes::<Test>::get(i).is_none());
        }
        assert_eq!(
            (
                crate::Nodes::<Test>::get(6),
                crate::Nodes::<Test>::get(9),
                crate::Nodes::<Test>::get(10),
                crate::RootHash::<Test>::get(),
            ),
            (
                Some(hex(
                    "d3abc9027d4a827c6acf44806672576b48a26e363a5d329b8e43327230a9f811"
                )),
                Some(hex(
                    "b2206bdb47aae933a96dd8860d23eb039d6add38cd85e892150e1c8c16831ba5"
                )),
                Some(hex(
                    "2762adf78b467c382b52cdb09cb8f885fea8719b1861369b4202497f645ad402"
                )),
                hex("87357c1450bf297e760b8545a7dff621b597a158fc510f14d7484778f7475513"),
            )
        );
    });
}

#[test]
fn should_calculate_the_size_correctly() {
    let _ = env_logger::try_init();

    let leaves = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 21];
    let sizes = vec![0, 1, 3, 4, 7, 8, 10, 11, 15, 16, 18, 19, 22, 23, 25, 26, 39];

    // size cross-check
    let mut actual_sizes = vec![];
    for s in &leaves[1..] {
        new_test_ext().execute_with(|| {
            let mut mmr = ModuleMmr::<mmr::storage::RuntimeStorage, crate::mock::Test, _>::new(0);
            for i in 0..*s {
                mmr.push(message(i));
            }
            actual_sizes.push(mmr.size());
        })
    }
    assert_eq!(sizes[1..], actual_sizes[..]);
}

#[test]
fn should_generate_verify_proofs_correctly() {
    let _ = env_logger::try_init();
    let mut ext = new_test_ext();
    // given
    let num: u64 = 7;
    ext.execute_with(|| {
        for id in 0..num {
            assert_ok!(HyperdriveOutgoing::send_message(action(id as u128)));

            // move to next block
            // this makes sure that 3 snapshots are taken, which is after 6 blocks for mock config with MaximumBlocksBeforeSnapshot==2
            next_block();
        }
        next_block();
        assert_eq!(8, System::block_number());
        assert_eq!(3, HyperdriveOutgoing::next_snapshot_number());
    });

    ext.persist_offchain_overlay();

    // Try to generate proofs now. This requires the offchain extensions to be present
    // to retrieve full leaf data.
    register_offchain_ext(&mut ext);
    ext.execute_with(|| {
        // when generate proofs for all leaves.
        let proofs = (0_u64..=num)
            .into_iter()
            .map(|next_message_number| {
                let p = Pallet::<Test>::generate_proof(next_message_number, None, 2).unwrap();
                if let Some((leaves, proof)) = p.clone() {
                    assert_eq!(Pallet::<Test>::verify_proof(leaves, proof), Ok(()));
                }
                p
            })
            .collect::<Vec<_>>();

        let _target_chain_proofs = (0_u64..=num)
            .into_iter()
            .map(|next_message_number| {
                Pallet::<Test>::generate_target_chain_proof(next_message_number, None, 2).unwrap()
            })
            .collect::<Vec<_>>();

        // when generate historical proofs for all leaves
        let historical_proofs = (0_u64..=num)
            .into_iter()
            .map(|next_message_number| {
                let mut proofs = vec![];
                for snapshot_number in 0..2 {
                    let p =
                        Pallet::<Test>::generate_proof(next_message_number, None, snapshot_number);
                    if let Ok(Some((leaves, proof))) = &p {
                        assert_eq!(
                            Pallet::<Test>::verify_proof(leaves.clone(), proof.clone()),
                            Ok(())
                        );
                    }
                    proofs.push(p)
                }
                proofs
            })
            .collect::<Vec<_>>();

        // then
        assert_eq!(
            proofs[0],
            Some((
                vec![
                    message(0),
                    message(1),
                    message(2),
                    message(3),
                    message(4),
                    message(5),
                    message(6)
                ],
                Proof {
                    leaf_indices: vec![0, 1, 2, 3, 4, 5, 6],
                    leaf_count: 7,
                    items: vec![],
                }
            ))
        );
        assert_eq!(
            historical_proofs[0][0],
            Ok(Some((
                vec![message(0), message(1), message(2),],
                Proof {
                    leaf_indices: vec![0, 1, 2],
                    leaf_count: 3,
                    items: vec![],
                }
            )))
        );

        //       D
        //     /   \
        //    /     \
        //   A       B       C
        //  / \     / \     / \
        // 0   1   2   3   4   5   6
        //         |-----proof-----|
        // proving 2 to 6 => we need blinded [A]
        assert_eq!(
            proofs[2],
            Some((
                vec![message(2), message(3), message(4), message(5), message(6)],
                Proof {
                    leaf_indices: vec![2, 3, 4, 5, 6],
                    leaf_count: 7,
                    items: vec![hex(
                        "bf9f9624420719df2fcbcfd14d0071aa8668c3433f6013bb253f4b4e03587b77"
                    ),],
                }
            ))
        );
        // generate proof for
        // * next_message_number=3 (we synchronized all messages up to 2)
        // * latest_known_snapshot_number=0 (at snapshot 0 there where only 3 leaves)
        //
        //   A
        //  / \
        // 1   2   3
        //
        // proving 3 => we need blinded [A]
        assert_eq!(
            historical_proofs[2][0],
            Ok(Some((
                vec![message(2)],
                Proof {
                    leaf_indices: vec![2],
                    leaf_count: 3,
                    items: vec![hex(
                        "bf9f9624420719df2fcbcfd14d0071aa8668c3433f6013bb253f4b4e03587b77"
                    )],
                }
            )))
        );
        // generate proof for
        // * next_message_number=3 (we synchronized all messages up to 2)
        // * latest_known_snapshot_number=1 (at snapshot 1 there where only 3 leaves)
        //
        //       D
        //     /   \
        //    /     \
        //   A       B
        //  / \     / \
        // 0   1   2   3   4
        //         |-proof-|
        // proving 2 to 4 => we need blinded [A]
        assert_eq!(
            historical_proofs[2][1],
            Ok(Some((
                vec![message(2), message(3), message(4)],
                Proof {
                    leaf_indices: vec![2, 3, 4],
                    leaf_count: 5,
                    items: vec![hex(
                        "bf9f9624420719df2fcbcfd14d0071aa8668c3433f6013bb253f4b4e03587b77"
                    ),],
                }
            )))
        );
        assert_eq!(historical_proofs[5][1], Ok(None));

        //       D
        //     /   \
        //    /     \
        //   A       B       C
        //  / \     / \     / \
        // 0   1   2   3   4   5   6
        //             |---proof---|
        // proving 3 to 6 => we need blinded [A, 2]
        assert_eq!(
            proofs[3],
            Some((
                vec![message(3), message(4), message(5), message(6)],
                Proof {
                    leaf_indices: vec![3, 4, 5, 6],
                    leaf_count: 7,
                    items: vec![
                        hex("0x4579a1e3d18b7dc5383a27b0f107cffb60ae2203460c7e85494e9ff6c36aece1"),
                        hex("0xbf9f9624420719df2fcbcfd14d0071aa8668c3433f6013bb253f4b4e03587b77")
                    ],
                }
            ))
        );
    });
}

#[test]
fn verification_should_be_stateless() {
    let _ = env_logger::try_init();
    let mut ext = new_test_ext();

    // Proof generation requires the offchain extensions to be present to retrieve full leaf data.
    register_offchain_ext(&mut ext);

    // given: start off with chain initialisation and storing indexing data off-chain (MMR Leafs)
    let root_a = ext.execute_with(|| {
        send_messages(6);
        // ensure snapshot is taken
        add_blocks(3);
        assert_eq!(1, HyperdriveOutgoing::next_snapshot_number());
        Pallet::<Test>::root_hash()
    });
    ext.persist_offchain_overlay();
    // when
    let (leaves_a, proof_a) =
        ext.execute_with(|| Pallet::<Test>::generate_proof(5, None, 0).unwrap().unwrap());

    // add more leaves which will change the on-chain root
    let root_b = ext.execute_with(|| {
        send_messages(1);
        // ensure snapshot is taken
        add_blocks(3);
        assert_eq!(2, HyperdriveOutgoing::next_snapshot_number());
        Pallet::<Test>::root_hash()
    });
    ext.persist_offchain_overlay();
    // when
    let (leaves_b, proof_b) =
        ext.execute_with(|| Pallet::<Test>::generate_proof(5, None, 1).unwrap().unwrap());

    // then: verify proof without relying on any on-chain data (stateless verification)
    assert_eq!(
        Pallet::<Test>::verify_proof_stateless(root_a, leaves_a, proof_a),
        Ok(())
    );
    assert_eq!(
        Pallet::<Test>::verify_proof_stateless(root_b, leaves_b, proof_b),
        Ok(())
    );
}

#[test]
fn should_generate_maximum_messages() {
    let _ = env_logger::try_init();
    let mut ext = new_test_ext();

    // Proof generation requires the offchain extensions to be present to retrieve full leaf data.
    register_offchain_ext(&mut ext);

    // given: start off with chain initialisation and storing indexing data off-chain (MMR Leafs)
    ext.execute_with(|| {
        send_messages(7);
        // ensure snapshot is taken
        add_blocks(3);
        assert_eq!(1, HyperdriveOutgoing::next_snapshot_number());
    });
    ext.persist_offchain_overlay();

    ext.execute_with(|| {
        // when: there are messages 2,3,4,5,6 to be prooved, but we limit the maximum messages to 3
        let (leaves, proof) = Pallet::<Test>::generate_proof(2, Some(3), 0)
            .unwrap()
            .unwrap();

        // then
        assert_eq!(leaves.len(), 3);
        assert_eq!(Pallet::<Test>::verify_proof(leaves, proof), Ok(()));
    });
}

// #[test]
// fn should_verify_canonicalized() {
//     use frame_support::traits::Hooks;
//     let _ = env_logger::try_init();
//
//     // How deep is our fork-aware storage (in terms of blocks/leaves, nodes will be more).
//     let block_hash_size: u64 = <Test as frame_system::Config>::BlockHashCount::get();
//
//     // Start off with chain initialisation and storing indexing data off-chain.
//     // Create twice as many leaf entries than our fork-aware capacity,
//     // resulting in ~half of MMR storage to use canonical keys and the other half fork-aware keys.
//     // Verify that proofs can be generated (using leaves and nodes from full set) and verified.
//     let mut ext = new_test_ext();
//     register_offchain_ext(&mut ext);
//     for blocknum in 0u32..(2 * block_hash_size).try_into().unwrap() {
//         ext.execute_with(|| {
//             next_block();
//             // this does currently noop, which is probably a mistake in original test!!
//             // TODO call the cannoncialize if moved to this repo, or move whole test to mmr-gadget repo
//             <Pallet<Test> as Hooks<BlockNumber>>::offchain_worker(blocknum.into());
//         });
//         ext.persist_offchain_overlay();
//     }
//
//     // Generate proofs for some blocks.
//     let (leaves, proofs) =
//         ext.execute_with(|| Pallet::<Test>::generate_proof(vec![1, 4, 5, 7], None).unwrap());
//     // Verify all previously generated proofs.
//     ext.execute_with(|| {
//         assert_eq!(Pallet::<Test>::verify_leaves(leaves, proofs), Ok(()));
//     });
//
//     // Generate proofs for some new blocks.
//     let (leaves, proofs) = ext.execute_with(|| {
//         Pallet::<Test>::generate_proof(vec![block_hash_size + 7], None).unwrap()
//     });
//     // Add some more blocks then verify all previously generated proofs.
//     ext.execute_with(|| {
//         send_messages(7);
//         assert_eq!(Pallet::<Test>::verify_leaves(leaves, proofs), Ok(()));
//     });
// }

#[test]
fn does_not_panic_when_generating_historical_proofs() {
    let _ = env_logger::try_init();
    let mut ext = new_test_ext();

    // given: start off with chain initialisation and storing indexing data off-chain (7 MMR Leafs)
    ext.execute_with(|| {
        send_messages(7);
        // ensure snapshot is taken
        add_blocks(3);
        assert_eq!(1, HyperdriveOutgoing::next_snapshot_number());
    });
    ext.persist_offchain_overlay();

    // Try to generate historical proof with invalid arguments. This requires the offchain
    // extensions to be present to retrieve full leaf data.
    register_offchain_ext(&mut ext);
    ext.execute_with(|| {
        // when next_message_number is in the future
        assert_eq!(
            Pallet::<Test>::generate_proof(9, None, 0),
            Err(MMRError::GenerateProofFutureMessage),
        );

        // when latest_known_snapshot is in the future
        assert_eq!(
            Pallet::<Test>::generate_proof(5, None, 1),
            Err(MMRError::GenerateProofFutureSnapshot),
        );

        // when no new messages since next_message_number-1
        assert_eq!(Pallet::<Test>::generate_proof(7, Some(0), 0), Ok(None));

        // when no new message because maximum_messages==0
        assert_eq!(Pallet::<Test>::generate_proof(5, Some(0), 0), Ok(None));
    });
}

/// Tests serialization for proof:
/// ```txt
/// k_index: 1, position: 8, message 05070700050707010000000641535349474e0a000000460507070a000000100000000000000000000000000000000502000000290a00000024747a316834457347756e48325565315432754e73386d664b5a38585a6f516a693348634b
/// k_index: 0, position: 10, message 05070700060707010000000641535349474e0a000000460507070a000000100000000000000000000000000000000602000000290a00000024747a316834457347756e48325565315432754e73386d664b5a38585a6f516a693348634b
///
/// mmr_size: 11
///
/// items: [0x53db3d426fa99eff2cc6ef1f07a226c2e5b32d9ccc2b67411d52e8d2b0de8d13,
///         0xbca5ce83486f6bd8be90523d0e9bcefd812fbd451337b584d32f8203dbf340c7]
/// ```
#[test]
fn should_serialize_target_chain_proof() {
    // given
    let proof = TargetChainProof {
        leaves: vec![TargetChainProofLeaf{
            k_index: 1,
            position: 8,
            message: hex!("05070700050707010000000641535349474e0a000000460507070a000000100000000000000000000000000000000502000000290a00000024747a316834457347756e48325565315432754e73386d664b5a38585a6f516a693348634b").into(),
        },
                     TargetChainProofLeaf{
                         k_index: 0,
                         position: 10,
                         message: hex!("05070700060707010000000641535349474e0a000000460507070a000000100000000000000000000000000000000602000000290a00000024747a316834457347756e48325565315432754e73386d664b5a38585a6f516a693348634b").into(),
                     }],
        mmr_size:11,
        items: vec![hex!("53db3d426fa99eff2cc6ef1f07a226c2e5b32d9ccc2b67411d52e8d2b0de8d13"), hex!("bca5ce83486f6bd8be90523d0e9bcefd812fbd451337b584d32f8203dbf340c7")],
    };

    // when
    let actual = serde_json::to_string(&proof).unwrap();

    // then
    assert_eq!(
        actual,
        r#"{"leaves":[{"kIndex":1,"position":8,"message":[5,7,7,0,5,7,7,1,0,0,0,6,65,83,83,73,71,78,10,0,0,0,70,5,7,7,10,0,0,0,16,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,5,2,0,0,0,41,10,0,0,0,36,116,122,49,104,52,69,115,71,117,110,72,50,85,101,49,84,50,117,78,115,56,109,102,75,90,56,88,90,111,81,106,105,51,72,99,75]},{"kIndex":0,"position":10,"message":[5,7,7,0,6,7,7,1,0,0,0,6,65,83,83,73,71,78,10,0,0,0,70,5,7,7,10,0,0,0,16,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,6,2,0,0,0,41,10,0,0,0,36,116,122,49,104,52,69,115,71,117,110,72,50,85,101,49,84,50,117,78,115,56,109,102,75,90,56,88,90,111,81,106,105,51,72,99,75]}],"mmrSize":11,"items":[[83,219,61,66,111,169,158,255,44,198,239,31,7,162,38,194,229,179,45,156,204,43,103,65,29,82,232,210,176,222,141,19],[188,165,206,131,72,111,107,216,190,144,82,61,14,155,206,253,129,47,189,69,19,55,181,132,211,47,130,3,219,243,64,199]]}"#
    );
}
