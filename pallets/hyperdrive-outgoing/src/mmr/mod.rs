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

use mmr_lib;
use mmr_lib::Merge;
use sp_runtime::traits;
use sp_std::prelude::ToOwned;

use crate::types::{Node, TargetChainConfig, TargetChainNodeHasher};
use crate::HashOf;

pub use self::mmr::{node_pos_to_k_index, verify_leaves_proof, Mmr};

mod mmr;
pub mod storage;

/// Node type for runtime `T`.
pub type NodeOf<T, I> = Node<HashOf<T, I>>;

/// Default Merging & Hashing behavior for MMR.
pub struct Merger<H: TargetChainConfig>(sp_std::marker::PhantomData<H>);

impl<H: TargetChainConfig> Merge for Merger<H> {
    type Item = Node<H::Hash>;
    fn merge(left: &Self::Item, right: &Self::Item) -> mmr_lib::Result<Self::Item> {
        let mut concat = H::hash_node(left)
            .map_err(|_| mmr_lib::Error::MergeError("hasher failed".to_owned()))?
            .as_ref()
            .to_vec();
        concat.extend_from_slice(
            H::hash_node(right)
                .map_err(|_| mmr_lib::Error::StoreError("hasher failed".to_owned()))?
                .as_ref(),
        );

        Ok(Node::Hash(<H::Hasher as traits::Hash>::hash(&concat)))
    }
}
