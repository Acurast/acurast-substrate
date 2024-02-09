use frame_benchmarking::benchmarks_instance_pallet;
use frame_benchmarking::whitelist_account;
use frame_benchmarking::whitelisted_caller;
use frame_support::assert_ok;
use frame_system::RawOrigin;
use sp_core::crypto::AccountId32;
use sp_core::H256;
use sp_std::{iter, prelude::*};

use crate::chain::tezos::TezosProof;
pub use crate::stub::*;
use crate::types::*;
use crate::Pallet as AcurastHyperdrive;
use core::marker::PhantomData;
use frame_system::pallet_prelude::BlockNumberFor;
use hex_literal::hex;

use super::*;

fn assert_last_event<T: Config<I>, I: 'static>(generic_event: <T as Config<I>>::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

fn assert_event<T: Config<I>, I: 'static>(generic_event: <T as Config<I>>::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_has_event(generic_event.into());
}

fn update_state_transmitters_helper<T: Config<I>, I: 'static>(
    l: usize,
    submit: bool,
) -> (T::AccountId, StateTransmitterUpdates<T>)
where
    T::AccountId: From<AccountId32>,
    BlockNumberFor<T>: From<u64>,
{
    let caller: T::AccountId = whitelisted_caller();
    whitelist_account!(caller);

    let actions = StateTransmitterUpdates::<T>::try_from(
        iter::repeat(StateTransmitterUpdate::Add(
            caller.clone(),
            ActivityWindow {
                start_block: 0.into(),
                end_block: 100.into(),
            },
        ))
        .take(l)
        .collect::<Vec<StateTransmitterUpdateFor<T>>>(),
    )
    .unwrap();

    if submit {
        let call = AcurastHyperdrive::<T, I>::update_state_transmitters(
            RawOrigin::Root.into(),
            actions.clone(),
        );
        assert_ok!(call);
    }

    (caller, actions)
}

benchmarks_instance_pallet! {
    where_clause {
        where
        T: Config<I>,
        T::AccountId: From<AccountId32>,
        BlockNumberFor<T>: From<u64>,
        T: Config<I, Proof = TezosProof<<T as Config<I>>::ParsableAccountId, <T as frame_system::Config>::AccountId>>,
        <T as pallet::Config<I>>::TargetChainBlockNumber: From<u64>,
        <T as pallet::Config<I>>::TargetChainHash: From<H256>,
    }
    update_state_transmitters {
        let l in 0 .. STATE_TRANSMITTER_UPDATES_MAX_LENGTH;

        // just create the data, do not submit the actual call (it gets executed by the benchmark call)
        let (account, actions) = update_state_transmitters_helper::<T, I>(l as usize, false);
    }: _(RawOrigin::Root, actions.clone())
    verify {
        assert_last_event::<T, I>(Event::StateTransmittersUpdate{
                    added: iter::repeat((
                            account.into(),
                            ActivityWindow {
                                start_block: 0.into(),
                                end_block: 100.into()
                            }
                        ))
                        .take(l as usize)
                        .collect::<Vec<(T::AccountId, ActivityWindow<BlockNumberFor<T>>)>>(),
                    updated: vec![],
                    removed: vec![],
                }.into());
    }

    submit_state_merkle_root {
        // add the transmitters and submit before benchmarked extrinsic
        let (caller, _) = update_state_transmitters_helper::<T, I>(1, true);
    }: _(RawOrigin::Signed(caller.clone()), 1.into(), HASH.into())
    verify {
         assert_event::<T, I>(Event::StateMerkleRootSubmitted{
                    source: caller.clone(),
                    snapshot: 1.into(),
                    state_merkle_root: HASH.into()
                }.into());
    }

    submit_message {
        <MessageSequenceId::<T, I>>::set(74);
        let (caller, _) = update_state_transmitters_helper::<T, I>(1, true);
        let proof_items: StateProof<H256> = vec![].try_into().unwrap();
        let key = StateKey::try_from(hex!("05008b01").to_vec()).unwrap();
        let value = StateValue::try_from(hex!("050707010000000c52454749535445525f4a4f4207070a00000016000016e64994c2ddbd293695b63e4cade029d3c8b5e30a000000ec050707030a0707050902000000250a00000020d80a8b0d800a3320528693947f7317871b2d51e5f3c8f3d0d4e4f7e6938ed68f070707070509020000002907070a00000020d80a8b0d800a3320528693947f7317871b2d51e5f3c8f3d0d4e4f7e6938ed68f00000707050900000707008080e898a9bf8d0700010707001d0707000107070001070702000000000707070700b40707070080cfb1eca062070700a0a9070707000000a0a5aaeca06207070a00000035697066733a2f2f516d536e317252737a444b354258634e516d4e367543767a4d376858636548555569426b61777758396b534d474b0000").to_vec()).unwrap();

        let proof = TezosProof::<<T as crate::Config<I>>::ParsableAccountId, <T as frame_system::Config>::AccountId> {
            items: proof_items,
            path: key,
            value,
            marker: PhantomData::default()
        };
        let snapshot_root_1 = H256(hex!(
            "8303857bb23c1b072d9b52409fffe7cf6de57c33b2776c7de170ec94d01f02fc"
        ));
        assert_ok!(AcurastHyperdrive::<T, I>::submit_state_merkle_root(RawOrigin::Signed(caller.clone()).into(), 1.into(), snapshot_root_1.into()));
        let state_owner = StateOwner::try_from(hex!("050a000000160199651cbe1a155a5c8e5af7d6ea5c3f48eebb8c9c00").to_vec()).unwrap();
        assert_ok!(AcurastHyperdrive::<T, I>::update_target_chain_owner(RawOrigin::Root.into(), state_owner));
    }: _(RawOrigin::Signed(caller), 1u8.into(), proof)

    update_target_chain_owner {
        let owner: StateOwner = state_owner();
    }: _(RawOrigin::Root, owner)

    impl_benchmark_test_suite!(AcurastHyperdrive, crate::mock::new_test_ext(), mock::Test);
}
