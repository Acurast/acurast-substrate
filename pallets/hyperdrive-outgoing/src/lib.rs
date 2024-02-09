// This file is part of Substrate.

// Copyright (C) 2020-2022 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![cfg_attr(not(feature = "std"), no_std)]

use core::cmp::min;
use core::ops::AddAssign;

use frame_support::dispatch::{Pays, PostDispatchInfo};
use frame_support::ensure;
use frame_system::pallet_prelude::{BlockNumberFor, HeaderFor};
use sp_core::Get;
use sp_runtime::traits::Block as BlockT;
use sp_runtime::traits::NumberFor;
use sp_runtime::traits::Saturating;
use sp_std::prelude::*;

use mmr_lib::leaf_index_to_pos;
pub use pallet::*;
pub use types::{
    Action, Leaf, LeafEncoder, LeafIndex, MMRError, Message, NodeIndex, OnNewRoot, Proof,
    RawAction, SnapshotNumber, TargetChainConfig, TargetChainProof,
};
pub use utils::NodesUtils;

pub use crate::default_weights::WeightInfo;
use crate::instances::HyperdriveInstance;
use crate::mmr::Merger;
use crate::traits::MMRInstance;
use crate::types::{Node, TargetChainProofLeaf};

#[cfg(test)]
pub mod mock;
#[cfg(any(test, feature = "runtime-benchmarks"))]
mod stub;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

mod default_weights;
pub use pallet_acurast_hyperdrive::instances;
pub mod chain;
mod mmr;
#[cfg(feature = "std")]
pub mod mmr_gadget;
#[cfg(feature = "std")]
pub mod rpc;
pub mod traits;
mod types;
pub mod utils;

/// A MMR specific to this pallet instance.
type ModuleMmr<StorageType, T, I> = mmr::Mmr<StorageType, T, I, Merger<TargetChainConfigOf<T, I>>>;

/// Hashing for target chain used for this pallet instance.
pub(crate) type TargetChainConfigOf<T, I> = <T as Config<I>>::TargetChainConfig;

/// Hash used for this pallet instance.
pub(crate) type HashOf<T, I> = <<T as Config<I>>::TargetChainConfig as TargetChainConfig>::Hash;

/// Encoder used for this pallet instance.
pub(crate) type TargetChainEncoderOf<T, I> =
    <<T as Config<I>>::TargetChainConfig as TargetChainConfig>::TargetChainEncoder;

/// Encoder error returned by Hasher/Encoder used for this pallet instance.
pub(crate) type HasherError<T, I> = <TargetChainEncoderOf<T, I> as LeafEncoder>::Error;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    use crate::default_weights::WeightInfo;
    use crate::traits::MMRInstance;

    use super::*;

    #[pallet::pallet]
    pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

    /// This pallet's configuration trait
    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config {
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type MMRInfo: MMRInstance;

        /// The bundled config of encoder/hasher using an encoding/hash function supported on target chain.
        type TargetChainConfig: TargetChainConfig;

        /// The usual number of blocks included before a new snapshot of the current MMR's [`RootHash`] is stored into [`SnapshotRootHash`].
        ///
        /// A snapshot can be delayed by more then the configured value if no messages get sent, but when a message is sent in block `b`,
        /// latest in block `b + MaximumBlocksBeforeSnapshot` a new snapshot will be taken.
        type MaximumBlocksBeforeSnapshot: Get<BlockNumberFor<Self>>;

        /// A hook to act on the new MMR root.
        ///
        /// For some applications it might be beneficial to make the MMR root available externally
        /// apart from having it in the storage. For instance you might output it in the header
        /// digest (see [`frame_system::Pallet::deposit_log`]) to make it available for Light
        /// Clients. Hook complexity should be `O(1)`.
        type OnNewRoot: OnNewRoot<HashOf<Self, I>>;

        /// Weights for this pallet.
        type WeightInfo: WeightInfo;
    }

    /// A tuple `(included_message_number_excl, next_message_number)`
    /// (where `next_message_number - 1` is not necessarily included in a snapshot).
    ///
    /// The [`next_message_number`] is strictly increasing, sequential order of messages sent.
    /// `next_message_number` is the ID for the next message sent.
    ///
    /// The relationship between blocks, messages=leaves and snapshots is sketched below:
    /// ```text
    /// |------block 3-----|  |---------block 4-------------|  |-------block 5---- - - -
    ///  m11       m12          m13   m14   m15   m16    m17     m18 m19
    /// -------------------------------------------snapshot-|  |------------------ - - -
    ///                                                          ↑   ↑
    ///                                    included_message_number   next_message_number-1
    /// ```
    #[pallet::storage]
    #[pallet::getter(fn message_numbers)]
    pub type MessageNumbers<T: Config<I>, I: 'static = ()> =
        StorageValue<_, (LeafIndex, LeafIndex), ValueQuery>;

    /// The block where the first MMR node was inserted.
    #[pallet::storage]
    #[pallet::getter(fn first_mmr_block_number)]
    pub type FirstMmrBlockNumber<T: Config<I>, I: 'static = ()> =
        StorageValue<_, BlockNumberFor<T>, OptionQuery>;

    /// An index `leaf_index -> (parent_block_hash, root_hash)`, where `root_hash` is the new root hash produced
    /// as a result of inserting leaf with `leaf_index`.
    ///
    /// Useful to recover the block hash of the parent that added a certain leaf.
    /// This block hash is used in temporary keys for offchain-indexing full leaves.
    #[pallet::storage]
    #[pallet::getter(fn leaf_meta)]
    pub type LeafMeta<T: Config<I>, I: 'static = ()> = StorageMap<
        _,
        Identity,
        LeafIndex,
        (<T as frame_system::Config>::Hash, HashOf<T, I>),
        OptionQuery,
    >;

    /// Index for `block -> last_message_excl`.
    ///
    /// Allows to retrieve the last message sent/leaf inserted during a (historic) block.
    /// Also allows to derive the range of messages/leaves insert during a (historic) block.
    #[pallet::storage]
    #[pallet::getter(fn block_leaf_index)]
    pub type BlockLeafIndex<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Identity, BlockNumberFor<T>, LeafIndex, OptionQuery>;

    /// Next snapshot number. The latest completed snapshot is the stored value - 1.
    #[pallet::storage]
    #[pallet::getter(fn next_snapshot_number)]
    pub type NextSnapshotNumber<T: Config<I>, I: 'static = ()> =
        StorageValue<_, SnapshotNumber, ValueQuery>;

    /// Meta data for a snapshot as a map `snapshot_number -> (root_hash, last_block, last_message_excl)`.
    ///
    /// First value is the latest snapshot's MMR root hash.
    ///
    /// `last_block` and `last_message_excl` are used to ensure a maximum number of blocks per snapshot, even if no messages get sent.
    #[pallet::storage]
    #[pallet::getter(fn snapshot_meta)]
    pub type SnapshotMeta<T: Config<I>, I: 'static = ()> = StorageMap<
        _,
        Identity,
        SnapshotNumber,
        (HashOf<T, I>, BlockNumberFor<T>, LeafIndex),
        OptionQuery,
    >;

    /// Latest MMR root hash.
    #[pallet::storage]
    #[pallet::getter(fn root_hash)]
    pub type RootHash<T: Config<I>, I: 'static = ()> = StorageValue<_, HashOf<T, I>, ValueQuery>;

    /// Current size of the MMR (number of leaves).
    #[pallet::storage]
    #[pallet::getter(fn number_of_leaves)]
    pub type NumberOfLeaves<T, I = ()> = StorageValue<_, LeafIndex, ValueQuery>;

    /// Hashes of the nodes in the MMR.
    ///
    /// Note this collection only contains MMR peaks, the inner nodes (and leaves)
    /// are pruned and only stored in the Offchain DB.
    #[pallet::storage]
    #[pallet::getter(fn mmr_peak)]
    pub type Nodes<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Identity, NodeIndex, HashOf<T, I>, OptionQuery>;

    #[pallet::hooks]
    impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {
        fn on_finalize(current_block: BlockNumberFor<T>) {
            let (included_message_number_excl, next_message_number) = Self::message_numbers();
            // check if we should create new snapshot
            if included_message_number_excl < next_message_number
                && Self::maximum_blocks_before_snapshot_reached(current_block)
            {
                // there was at least one message since last snapshot and enough blocks passed -> take snapshot
                let current_snapshot = <NextSnapshotNumber<T, I>>::mutate(|s| {
                    let current_snapshot = *s;
                    s.add_assign(1);
                    current_snapshot
                });
                SnapshotMeta::<T, I>::insert(
                    current_snapshot,
                    (RootHash::<T, I>::get(), current_block, next_message_number),
                );
                MessageNumbers::<T, I>::put((next_message_number, next_message_number));
            }

            // always update the block-leaf-index (also when not taking a snapshot)
            BlockLeafIndex::<T, I>::insert(current_block, next_message_number);

            if Self::first_mmr_block_number() == None {
                <FirstMmrBlockNumber<T, I>>::put(current_block);
            }
        }

        fn on_initialize(current_block: BlockNumberFor<T>) -> Weight {
            // add weight used here in on_finalize

            // We did the check already and will repeat it in on_finalize
            let weight = T::WeightInfo::check_snapshot().saturating_mul(2);

            // predict if we definitely will not create snapshot in the initialized block and estimate lower weight
            if !Self::maximum_blocks_before_snapshot_reached(current_block) {
                return weight;
            }

            // we can't avoid that sometimes there is no message at the end of MaximumBlocksBeforeSnapshot
            // and we unnecessarily reserve weight for snapshotting
            weight.saturating_add(T::WeightInfo::create_snapshot())
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        /// A message was successfully sent. [JobId, SourceId, Assignment]
        MessageSent(Message),
    }

    #[pallet::error]
    pub enum Error<T, I = ()> {
        MMRPush,
    }

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::send_message())]
        pub fn send_test_message(origin: OriginFor<T>) -> DispatchResult {
            ensure_root(origin)?;

            Self::send_message(Action::Noop).map_err(|e| {
                e.log_error("send_message failed");
                Error::<T, I>::MMRPush
            })?;

            Ok(().into())
        }
    }
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
    /// Sends a message with the given [`Action`] over Hyperdrive.
    pub fn send_message(action: Action) -> Result<PostDispatchInfo, MMRError> {
        let leaves = Self::number_of_leaves();
        // used to calculate actual weight, see below
        let peaks_before = NodesUtils::new(leaves).number_of_peaks();

        let (included_message_number_excl, next_message_number) = Self::message_numbers();
        let message = Message {
            id: next_message_number,
            action,
        };

        // append new leaf to MMR
        let mut mmr: ModuleMmr<mmr::storage::RuntimeStorage, T, I> = mmr::Mmr::new(leaves);
        // MMR push never fails, but better safe than sorry.
        mmr.push(message.clone()).ok_or(MMRError::Push)?;
        // Update the size, `mmr.finalize()` should also never fail.
        let (leaves, root) = mmr.finalize()?;
        <T::OnNewRoot as OnNewRoot<_>>::on_new_root(&root);

        <NumberOfLeaves<T, I>>::put(leaves);
        <RootHash<T, I>>::put(root);
        MessageNumbers::<T, I>::put((included_message_number_excl, next_message_number + 1));

        Self::deposit_event(Event::MessageSent(message));

        // use peaks_after - peaks_before difference to calculate actual weight
        let peaks_after = NodesUtils::new(leaves).number_of_peaks();
        Ok(PostDispatchInfo {
            actual_weight: Some(T::WeightInfo::send_message_actual_weight(
                peaks_before.max(peaks_after),
            )),
            pays_fee: Pays::Yes,
        })
    }

    /// Build offchain key from `parent_hash` of block that originally added node `pos` to MMR.
    ///
    /// This combination makes the offchain (key, value) entry resilient to chain forks.
    fn node_temp_offchain_key(
        pos: NodeIndex,
        parent_hash: <T as frame_system::Config>::Hash,
        unique: HashOf<T, I>,
    ) -> Vec<u8> {
        NodesUtils::node_temp_offchain_key::<HeaderFor<T>, _>(
            &T::MMRInfo::TEMP_INDEXING_PREFIX,
            pos,
            parent_hash,
            unique,
        )
    }

    /// Build canonical offchain key for node `pos` in MMR.
    ///
    /// Used for nodes added by now finalized blocks.
    /// Never read keys using `node_canon_offchain_key` unless you sure that
    /// there's no `node_offchain_key` key in the storage.
    fn node_canon_offchain_key(pos: NodeIndex) -> Vec<u8> {
        NodesUtils::node_canon_offchain_key(&T::MMRInfo::INDEXING_PREFIX, pos)
    }

    /// Check if we should create new snapshot at the end of `current_block`,
    /// according to [`T::MaximumBlocksBeforeSnapshot`].
    ///
    /// This function should be combined with a check (not included!) if there was at least one new message to snapshot.
    fn maximum_blocks_before_snapshot_reached(current_block: BlockNumberFor<T>) -> bool {
        if let Some(first_block_number) = Self::first_mmr_block_number() {
            // there was at least one message/leaf inserted (not necessarily snapshotted)
            let last_block = Self::snapshot_meta(Self::next_snapshot_number().saturating_sub(1))
                .map(|(_root_hash, last_block, _last_message_excl)| last_block)
                .unwrap_or(first_block_number);
            current_block.saturating_sub(last_block) >= T::MaximumBlocksBeforeSnapshot::get().into()
        } else {
            false
        }
    }

    /// Generates a MMR proof for the messages in the range `[next_message_number..last_message_excl]`.
    ///
    /// If `next_message_number` is not yet sent, an error is returned.
    /// `last_message_excl` is the exclusive upper bound of messages to transmit and is bounded by latest message's index.
    /// If `maximum_messages` is provided, `next_message_number + maximum_messages` it the potentially lower bound used to
    /// limit the number of messages transferred at once.
    ///
    /// The proof is generated for the root at the end of the block that also produced the snapshot with `latest_known_snapshot_number`.
    ///
    /// If no new messages exist that have to be transmitted or they are not included in snapshot with `latest_known_snapshot_number`,
    /// this function returns `Ok(None)`.
    ///
    /// Note this function can only be used from an off-chain context
    /// (Offchain Worker or Runtime API call), since it requires
    /// all the leaves to be present.
    /// It may return an error or panic if used incorrectly.
    pub fn generate_proof(
        next_message_number: LeafIndex,
        maximum_messages: Option<u64>,
        latest_known_snapshot_number: SnapshotNumber,
    ) -> Result<Option<(Vec<Leaf>, Proof<HashOf<T, I>>)>, MMRError> {
        let (_root_hash, _last_block, last_message_excl) =
            Self::snapshot_meta(latest_known_snapshot_number)
                .ok_or(MMRError::GenerateProofFutureSnapshot)?;

        ensure!(
            next_message_number <= last_message_excl,
            MMRError::GenerateProofFutureMessage
        );

        let last_message_excl = if let Some(maximum) = maximum_messages {
            min(last_message_excl, next_message_number + maximum)
        } else {
            last_message_excl
        };

        if next_message_number == last_message_excl {
            // no new messages to transmit
            return Ok(None);
        }

        // since we create one leaf per message, the number of leaves at the end of the block where latest_known_snapshot_number
        // was taken is equal to the messages included at that time which is equal to last_message_excl
        let leaves_count = last_message_excl;
        // retrieve proof for the leaf index range [next_message_number..last_message_excl]
        let mmr: ModuleMmr<mmr::storage::OffchainStorage, T, I> = mmr::Mmr::new(leaves_count);
        mmr.generate_proof((next_message_number..last_message_excl).collect())
            .map(|result| Some(result))
    }

    /// Generates a self-contained MMR proof for the messages in the range `[next_message_number..last_message_excl]`.
    /// Leaves with their leaf index and position are part of the proof structure and contain the message encoded for the target chain.
    ///
    /// This function wraps [`Self::generate_proof`] and converts result to [`TargetChainProof`].
    pub fn generate_target_chain_proof(
        next_message_number: LeafIndex,
        maximum_messages: Option<u64>,
        latest_known_snapshot_number: SnapshotNumber,
    ) -> Result<Option<TargetChainProof<HashOf<T, I>>>, MMRError> {
        let proof = Self::generate_proof(
            next_message_number,
            maximum_messages,
            latest_known_snapshot_number,
        )?;
        proof
            .map(|(leaves, proof)| {
                let mmr_size = NodesUtils::new(proof.leaf_count).size();
                let leaf_positions: Vec<NodeIndex> = proof
                    .leaf_indices
                    .iter()
                    .map(|leaf_index| leaf_index_to_pos(leaf_index.to_owned()))
                    .collect();
                let leaf_k_indices = mmr::node_pos_to_k_index(leaf_positions.clone(), mmr_size);
                let leaves = leaf_positions
                    .iter()
                    .zip(leaf_k_indices.iter())
                    .zip(leaves.iter())
                    .map(|((position, (pos, k_index)), leaf)| {
                        assert_eq!(pos, position);
                        Ok(TargetChainProofLeaf {
                            k_index: k_index.to_owned() as NodeIndex,
                            position: position.to_owned(),
                            message: TargetChainEncoderOf::<T, I>::encode(leaf)
                                .map_err(|_| MMRError::GenerateProof)?,
                        })
                    })
                    .collect::<Result<Vec<TargetChainProofLeaf>, MMRError>>()?;
                Ok(TargetChainProof {
                    leaves,
                    mmr_size,
                    items: proof.items,
                })
            })
            .transpose()
    }

    /// Returns the snapshot MMR roots from `next_expected_snapshot_number, ...` onwards or an empty vec if no new snapshots.
    pub fn snapshot_roots(
        next_expected_snapshot_number: SnapshotNumber,
    ) -> impl Iterator<Item = Result<(SnapshotNumber, HashOf<T, I>), MMRError>> + 'static {
        let next_snapshot_number = Self::next_snapshot_number();
        (next_expected_snapshot_number..next_snapshot_number)
            .into_iter()
            .map(move |snapshot_number| {
                if let Some((root_hash, _last_block, _last_message_excl)) =
                    Self::snapshot_meta(snapshot_number)
                {
                    Ok((snapshot_number, root_hash))
                } else {
                    Err(MMRError::InconsistentSnapshotMeta)
                }
            })
    }

    /// Verify MMR proof for given `leaves`.
    ///
    /// This method is safe to use within the runtime code.
    /// It will return `Ok(())` if the proof is valid
    /// and an `Err(..)` if MMR is inconsistent (some leaves are missing)
    /// or the proof is invalid.
    pub fn verify_proof(leaves: Vec<Leaf>, proof: Proof<HashOf<T, I>>) -> Result<(), MMRError> {
        if proof.leaf_count > Self::number_of_leaves()
            || proof.leaf_count == 0
            || (proof.items.len().saturating_add(leaves.len())) as u64 > proof.leaf_count
        {
            return Err(MMRError::Verify
                .log_debug("The proof has incorrect number of leaves or proof items."));
        }

        let mmr: ModuleMmr<mmr::storage::OffchainStorage, T, I> = mmr::Mmr::new(proof.leaf_count);
        let is_valid = mmr.verify_leaves_proof(leaves, proof)?;
        if is_valid {
            Ok(())
        } else {
            Err(MMRError::Verify.log_debug("The proof is incorrect."))
        }
    }

    /// Stateless MMR proof verification for batch of leaves.
    ///
    /// This function can be used to verify received MMR [`Proof`] (`proof`)
    /// for given leaves set (`leaves`) against a known MMR root hash (`root`).
    /// Note, the leaves should be sorted such that corresponding leaves and leaf indices have the
    /// same position in both the `leaves` vector and the `leaf_indices` vector contained in the
    /// [`Proof`].
    pub fn verify_proof_stateless(
        root: HashOf<T, I>,
        leaves: Vec<Leaf>,
        proof: Proof<HashOf<T, I>>,
    ) -> Result<(), MMRError> {
        let is_valid = mmr::verify_leaves_proof::<T, I, Merger<TargetChainConfigOf<T, I>>>(
            root,
            leaves.iter().map(|leaf| Node::Data(leaf.clone())).collect(),
            proof,
        )?;
        if is_valid {
            Ok(())
        } else {
            Err(MMRError::Verify.log_debug(("The proof is incorrect.", root)))
        }
    }
}

sp_api::decl_runtime_apis! {
    /// API to interact with MMR pallet.
    pub trait HyperdriveApi<MmrHash: codec::Codec> {
        /// Return the number of MMR leaves/messages on-chain.
        fn number_of_leaves(instance: HyperdriveInstance) -> LeafIndex;

        fn first_mmr_block_number(instance: HyperdriveInstance) -> Option<NumberFor<Block>>;

        fn leaf_meta(instance: HyperdriveInstance, leaf_index: LeafIndex) -> Option<(<Block as BlockT>::Hash, MmrHash)>;

        fn last_message_excl_by_block(instance: HyperdriveInstance, block_number: NumberFor<Block>) -> Option<LeafIndex>;

        fn snapshot_roots(instance: HyperdriveInstance, next_expected_snapshot_number: SnapshotNumber) -> Result<Vec<(SnapshotNumber, MmrHash)>, MMRError>;

        fn snapshot_root(instance: HyperdriveInstance, next_expected_snapshot_number: SnapshotNumber) -> Result<Option<(SnapshotNumber, MmrHash)>, MMRError>;

        /// Generates a self-contained MMR proof for the messages in the range `[next_message_number..last_message_excl]`.
        /// Leaves with their leaf index and position are part of the proof structure and contain the message encoded for the target chain.
        ///
        /// This function forwards to [`Pallet::generate_target_chain_proof`].
        fn generate_target_chain_proof(
            instance: HyperdriveInstance,
            next_message_number: LeafIndex,
            maximum_messages: Option<u64>,
            latest_known_snapshot_number: SnapshotNumber,
        ) -> Result<Option<TargetChainProof<MmrHash>>, MMRError>;
    }
}
