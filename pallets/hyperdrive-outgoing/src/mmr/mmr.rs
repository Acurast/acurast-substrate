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

use sp_std::prelude::*;

use crate::mmr::HashOf;
use crate::types::{Proof, TargetChainNodeHasher};
use crate::utils::NodesUtils;
use crate::{
    mmr::{
        storage::{OffchainStorage, RuntimeStorage, Storage},
        Node,
    },
    types::{MMRError, NodeIndex},
    Config, HasherError, Leaf, TargetChainConfigOf,
};
use mmr_lib;
use mmr_lib::helper;
use mmr_lib::Merge;

/// Stateless verification of the proof for a batch of leaves.
/// Note, the leaves should be sorted such that corresponding leaves and leaf indices have the
/// same position in both the `leaves` vector and the `leaf_indices` vector contained in the
/// [primitives::Proof]
pub fn verify_leaves_proof<T, I, M>(
    root: HashOf<T, I>,
    leaves: Vec<Node<HashOf<T, I>>>,
    proof: Proof<HashOf<T, I>>,
) -> Result<bool, MMRError>
where
    T: Config<I>,
    M: Merge<Item = Node<HashOf<T, I>>>,
{
    let size = NodesUtils::new(proof.leaf_count).size();

    if leaves.len() != proof.leaf_indices.len() {
        return Err(MMRError::Verify.log_debug("Proof leaf_indices not same length with leaves"));
    }

    let leaves_and_position_data = proof
        .leaf_indices
        .into_iter()
        .map(|index| mmr_lib::leaf_index_to_pos(index))
        .zip(leaves.into_iter())
        .collect();

    let p = mmr_lib::MerkleProof::<Node<HashOf<T, I>>, M>::new(
        size,
        proof.items.into_iter().map(Node::Hash).collect(),
    );
    p.verify(Node::Hash(root), leaves_and_position_data)
        .map_err(|e| MMRError::Verify.log_debug(e))
}

/// A wrapper around an MMR library to expose limited functionality.
///
/// Available functions depend on the storage kind ([Runtime](crate::mmr::storage::RuntimeStorage)
/// vs [Off-chain](crate::mmr::storage::OffchainStorage)).
pub struct Mmr<StorageType, T, I, M>
where
    T: Config<I>,
    I: 'static,
    Storage<StorageType, T, I>: mmr_lib::MMRStoreReadOps<Node<HashOf<T, I>>>,
    M: Merge<Item = Node<HashOf<T, I>>>,
{
    mmr: mmr_lib::MMR<Node<HashOf<T, I>>, M, Storage<StorageType, T, I>, HashOf<T, I>>,
    leaves: NodeIndex,
}

impl<StorageType, T, I, M> Mmr<StorageType, T, I, M>
where
    T: Config<I>,
    I: 'static,
    Storage<StorageType, T, I>: mmr_lib::MMRStoreReadOps<Node<HashOf<T, I>>>,
    M: Merge<Item = Node<HashOf<T, I>>>,
{
    /// Create a pointer to an existing MMR with given number of leaves.
    pub fn new(leaves: NodeIndex) -> Self {
        let size = NodesUtils::new(leaves).size();
        Self {
            mmr: mmr_lib::MMR::new(size, Default::default()),
            leaves,
        }
    }

    /// Verify proof for a set of leaves.
    /// Note, the leaves should be sorted such that corresponding leaves and leaf indices have
    /// the same position in both the `leaves` vector and the `leaf_indices` vector contained in the
    /// [primitives::Proof]
    pub fn verify_leaves_proof(
        &self,
        leaves: Vec<Leaf>,
        proof: Proof<HashOf<T, I>>,
    ) -> Result<bool, MMRError> {
        let p = mmr_lib::MerkleProof::<Node<HashOf<T, I>>, M>::new(
            self.mmr.mmr_size(),
            proof.items.into_iter().map(Node::Hash).collect(),
        );

        if leaves.len() != proof.leaf_indices.len() {
            return Err(
                MMRError::Verify.log_debug("Proof leaf_indices not same length with leaves")
            );
        }

        let leaves_positions_and_data = proof
            .leaf_indices
            .into_iter()
            .map(|index| mmr_lib::leaf_index_to_pos(index))
            .zip(leaves.into_iter().map(|leaf| Node::Data(leaf)))
            .collect();
        let root = self
            .mmr
            .get_root()
            .map_err(|e| MMRError::GetRoot.log_error(e))?;
        p.verify(root, leaves_positions_and_data)
            .map_err(|e| MMRError::Verify.log_debug(e))
    }

    /// Return the internal size of the MMR (number of nodes).
    #[cfg(test)]
    pub fn size(&self) -> NodeIndex {
        self.mmr.mmr_size()
    }
}

/// Runtime specific MMR functions.
impl<T, I, M> Mmr<RuntimeStorage, T, I, M>
where
    T: Config<I>,
    I: 'static,
    M: Merge<Item = Node<HashOf<T, I>>>,
{
    /// Push another item to the MMR.
    ///
    /// Returns element position (index) in the MMR.
    pub fn push(&mut self, leaf: Leaf) -> Option<NodeIndex> {
        let position = self
            .mmr
            .push(Node::Data(leaf))
            .map_err(|e| MMRError::Push.log_error(e))
            .ok()?;

        self.leaves += 1;

        Some(position)
    }

    /// Commit the changes to underlying storage, return current number of leaves and
    /// calculate the new MMR's root hash.
    pub fn finalize(mut self) -> Result<(NodeIndex, HashOf<T, I>), MMRError> {
        let root = self
            .mmr
            .get_root()
            .map_err(|e| MMRError::GetRoot.log_error(e))?;
        let root_hash = TargetChainConfigOf::<T, I>::hash_node(&root)
            .map_err(|e| MMRError::Commit.log_error(e))?;
        self.mmr
            .commit(&root_hash)
            .map_err(|e| MMRError::Commit.log_error(e))?;
        Ok((self.leaves, root_hash))
    }
}

/// Off-chain specific MMR functions.
impl<T, I, M> Mmr<OffchainStorage, T, I, M>
where
    T: Config<I>,
    I: 'static,
    M: Merge<Item = Node<HashOf<T, I>>>,
{
    /// Generate a proof for given leaf indices.
    ///
    /// Proof generation requires all the nodes (or their hashes) to be available in the storage.
    /// (i.e. you can't run the function in the pruned storage).
    pub fn generate_proof(
        &self,
        leaf_indices: Vec<NodeIndex>,
    ) -> Result<(Vec<Leaf>, Proof<HashOf<T, I>>), MMRError> {
        let positions = leaf_indices
            .iter()
            .map(|index| mmr_lib::leaf_index_to_pos(*index))
            .collect::<Vec<_>>();
        let store = <Storage<OffchainStorage, T, I>>::default();
        let leaves = positions
            .iter()
            .map(
                |pos| match mmr_lib::MMRStoreReadOps::get_elem(&store, *pos) {
                    Ok(Some(Node::Data(leaf))) => Ok(leaf),
                    e => Err(MMRError::LeafNotFound.log_error(e)),
                },
            )
            .collect::<Result<Vec<_>, MMRError>>()?;

        let leaf_count = self.leaves;
        let proof = self
            .mmr
            .gen_proof(positions)
            .map_err(|e| MMRError::GenerateProof.log_error(e))?;

        Ok((
            leaves,
            Proof {
                leaf_indices,
                leaf_count,
                items: proof
                    .proof_items()
                    .iter()
                    .map(|x| TargetChainConfigOf::<T, I>::hash_node(x))
                    .collect::<Result<Vec<HashOf<T, I>>, HasherError<T, I>>>()
                    .map_err(|e| MMRError::GenerateProof.log_error(e))?,
            },
        ))
    }
}

pub fn node_pos_to_k_index(mut leaf_positions: Vec<u64>, mmr_size: u64) -> Vec<(u64, usize)> {
    let peaks = helper::get_peaks(mmr_size);
    let mut leaves_with_k_indices = vec![];

    for peak in peaks {
        let leaves: Vec<_> = take_while_vec(&mut leaf_positions, |pos| *pos <= peak);

        if leaves.len() > 0 {
            for pos in leaves {
                let height = helper::pos_height_in_tree(peak);
                let mut index = 0;
                let mut parent_pos = peak;
                for height in (1..=height).rev() {
                    let left_child = parent_pos - helper::parent_offset(height - 1);
                    let right_child = left_child + helper::sibling_offset(height - 1);
                    index *= 2;
                    if left_child >= pos {
                        parent_pos = left_child;
                    } else {
                        parent_pos = right_child;
                        index += 1;
                    }
                }

                leaves_with_k_indices.push((pos, index));
            }
        }
    }

    leaves_with_k_indices
}

fn take_while_vec<T, P: Fn(&T) -> bool>(v: &mut Vec<T>, p: P) -> Vec<T> {
    for i in 0..v.len() {
        if !p(&v[i]) {
            return v.drain(..i).collect();
        }
    }
    v.drain(..).collect()
}
