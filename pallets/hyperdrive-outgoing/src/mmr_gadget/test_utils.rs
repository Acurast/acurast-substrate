// This file is part of Substrate.

// Copyright (C) 2021-2022 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use std::collections::HashMap;
use std::{future::Future, sync::Arc, time::Duration};

use pallet_acurast_hyperdrive::instances::{HyperdriveInstance, TezosInstance};
use parking_lot::Mutex;
use sc_block_builder::BlockBuilderProvider;
use sc_client_api::{
    Backend as BackendT, BlockchainEvents, FinalityNotifications, ImportNotifications,
    StorageEventStream, StorageKey,
};
use sc_offchain::OffchainDb;
use sp_api::{ApiRef, ProvideRuntimeApi};
use sp_blockchain::{BlockStatus, CachedHeaderMetadata, HeaderBackend, HeaderMetadata, Info};
use sp_consensus::BlockOrigin;
use sp_core::{
    offchain::{DbExternalities, StorageKind},
    H256 as MmrHash,
};
use sp_runtime::traits::NumberFor;
use sp_runtime::{
    generic::BlockId,
    traits::{Block as BlockT, Header as HeaderT},
};
use substrate_test_runtime_client::{
    runtime::{Block, BlockNumber, Hash, Header},
    Backend, BlockBuilderExt, Client, ClientBlockImportExt, ClientExt, DefaultTestClientBuilderExt,
    TestClientBuilder, TestClientBuilderExt,
};
use tokio::runtime::Runtime;

use crate::mmr_gadget::MmrGadget;
use crate::{
    utils::NodesUtils, HyperdriveApi, LeafIndex, MMRError, NodeIndex, SnapshotNumber,
    TargetChainProof,
};

pub(crate) struct MockRuntimeApiData {
    pub(crate) num_blocks: BlockNumber,
    pub(crate) num_leaves: LeafIndex,
    pub(crate) last_message_excl_by_block: HashMap<BlockNumber, LeafIndex>,
    pub(crate) leaf_meta: HashMap<LeafIndex, (Hash, MmrHash)>,
}

#[derive(Clone)]
pub(crate) struct MockRuntimeApi {
    pub(crate) data: Arc<Mutex<MockRuntimeApiData>>,
}

impl MockRuntimeApi {
    pub(crate) const INDEXING_PREFIX: &'static [u8] = b"mmr-test-";
    pub(crate) const TEMP_INDEXING_PREFIX: &'static [u8] = b"mmr-test-temp-";
}

#[derive(Clone, Debug)]
pub(crate) struct MmrBlock {
    pub(crate) block: Block,
    pub(crate) leaf_indices: Vec<LeafIndex>,
}

#[derive(Clone, Copy)]
pub enum OffchainKeyType {
    Temp,
    Canon,
}

impl MmrBlock {
    pub fn hash(&self) -> Hash {
        self.block.hash()
    }

    pub fn parent_hash(&self) -> Hash {
        *self.block.header.parent_hash()
    }

    pub fn get_offchain_key(
        &self,
        node: NodeIndex,
        unique: Hash,
        key_type: OffchainKeyType,
    ) -> Vec<u8> {
        match key_type {
            OffchainKeyType::Temp => NodesUtils::node_temp_offchain_key::<Header, Hash>(
                MockRuntimeApi::TEMP_INDEXING_PREFIX,
                node,
                self.parent_hash(),
                unique,
            ),
            OffchainKeyType::Canon => {
                NodesUtils::node_canon_offchain_key(MockRuntimeApi::INDEXING_PREFIX, node)
            }
        }
    }
}

pub(crate) struct MockClient {
    pub(crate) client: Mutex<Client<Backend>>,
    pub(crate) backend: Arc<Backend>,
    pub(crate) runtime_api_params: Arc<Mutex<MockRuntimeApiData>>,
}

impl MockClient {
    pub(crate) fn new() -> Self {
        let client_builder = TestClientBuilder::new().enable_offchain_indexing_api();
        let (client, backend) = client_builder.build_with_backend();
        MockClient {
            client: Mutex::new(client),
            backend,
            runtime_api_params: Arc::new(Mutex::new(MockRuntimeApiData {
                num_blocks: 0,
                num_leaves: 0,
                last_message_excl_by_block: Default::default(),
                leaf_meta: Default::default(),
            })),
        }
    }

    pub(crate) fn offchain_db(&self) -> OffchainDb<<Backend as BackendT<Block>>::OffchainStorage> {
        OffchainDb::new(self.backend.offchain_storage().unwrap())
    }

    pub async fn import_block(
        &self,
        at: &BlockId<Block>,
        name: &[u8],
        leaf_indices: Vec<LeafIndex>,
    ) -> MmrBlock {
        let mut client = self.client.lock();
        let parent = client.block_hash_from_id(at).unwrap();
        let mut block_builder = if let Some(parent) = parent {
            client
                .new_block_at(parent, Default::default(), false)
                .unwrap()
        } else {
            client.new_block(Default::default()).unwrap()
        };
        // Make sure the block has a different hash than its siblings
        block_builder
            .push_storage_change(b"name".to_vec(), Some(name.to_vec()))
            .unwrap();
        let block = block_builder.build().unwrap().block;
        client
            .import(BlockOrigin::Own, block.clone())
            .await
            .unwrap();

        let parent_hash = *block.header.parent_hash();
        // Simulate writing MMR nodes in offchain storage
        for leaf_index in leaf_indices.iter().cloned() {
            let mut runtime_api = self.runtime_api_params.lock();
            runtime_api.num_leaves += 1;
            runtime_api
                .last_message_excl_by_block
                .insert(block.header.number, leaf_index + 1);
            runtime_api.leaf_meta.insert(
                leaf_index,
                (parent_hash, MmrHash::from_low_u64_be(leaf_index)),
            );

            let mut offchain_db = self.offchain_db();
            for node in NodesUtils::right_branch_ending_in_leaf(leaf_index) {
                let temp_key = NodesUtils::node_temp_offchain_key::<Header, Hash>(
                    MockRuntimeApi::TEMP_INDEXING_PREFIX,
                    node,
                    parent_hash,
                    MmrHash::from_low_u64_be(leaf_index),
                );
                offchain_db.local_storage_set(
                    StorageKind::PERSISTENT,
                    &temp_key,
                    &leaf_index.to_be_bytes(),
                )
            }
        }

        MmrBlock {
            block,
            leaf_indices,
        }
    }

    pub fn finalize_block(&self, hash: Hash) {
        let client = self.client.lock();
        client.finalize_block(hash, None).unwrap();
    }

    pub fn check_offchain_storage<F>(
        &self,
        key_type: OffchainKeyType,
        blocks: &[&MmrBlock],
        mut f: F,
    ) where
        F: FnMut(Option<Vec<u8>>, &MmrBlock, LeafIndex),
    {
        let mut offchain_db = self.offchain_db();
        for mmr_block in blocks {
            for leaf_index in mmr_block.leaf_indices.iter().cloned() {
                for node in NodesUtils::right_branch_ending_in_leaf(leaf_index) {
                    let last_message_excl = self
                        .runtime_api_params
                        .lock()
                        .last_message_excl_by_block
                        .get(&mmr_block.block.header.number)
                        .copied()
                        .expect("expected entry in on-chain index (mocked)");

                    let temp_key = mmr_block.get_offchain_key(
                        node,
                        MmrHash::from_low_u64_be(last_message_excl - 1),
                        key_type,
                    );
                    let val = offchain_db.local_storage_get(StorageKind::PERSISTENT, &temp_key);
                    f(val, mmr_block, leaf_index);
                }
            }
        }
    }

    pub fn assert_pruned(&self, _blocks: &[&MmrBlock]) {
        // TODO uncomment when pruning is implemented
        // self.check_offchain_storage(OffchainKeyType::Temp, blocks, |val, _block, _leaf_index| {
        //     assert!(val.is_none());
        // })
    }

    pub fn assert_not_pruned(&self, blocks: &[&MmrBlock]) {
        self.check_offchain_storage(OffchainKeyType::Temp, blocks, |val, _block, leaf_index| {
            assert_eq!(val, Some(leaf_index.to_be_bytes().to_vec()));
        })
    }

    pub fn assert_canonicalized(&self, blocks: &[&MmrBlock]) {
        self.check_offchain_storage(OffchainKeyType::Canon, blocks, |val, _block, leaf_index| {
            assert_eq!(val, Some(leaf_index.to_be_bytes().to_vec()));
        });

        self.assert_pruned(blocks);
    }

    pub fn assert_not_canonicalized(&self, blocks: &[&MmrBlock]) {
        self.check_offchain_storage(
            OffchainKeyType::Canon,
            blocks,
            |val, _block, _leaf_index| {
                assert!(val.is_none());
            },
        );

        self.assert_not_pruned(blocks);
    }
}

impl HeaderMetadata<Block> for MockClient {
    type Error = <Client<Backend> as HeaderMetadata<Block>>::Error;

    fn header_metadata(&self, hash: Hash) -> Result<CachedHeaderMetadata<Block>, Self::Error> {
        self.client.lock().header_metadata(hash)
    }

    fn insert_header_metadata(&self, _hash: Hash, _header_metadata: CachedHeaderMetadata<Block>) {
        unimplemented!()
    }

    fn remove_header_metadata(&self, _hash: Hash) {
        unimplemented!()
    }
}

impl HeaderBackend<Block> for MockClient {
    fn header(&self, hash: Hash) -> sc_client_api::blockchain::Result<Option<Header>> {
        self.client.lock().header(hash)
    }

    fn info(&self) -> Info<Block> {
        self.client.lock().info()
    }

    fn status(&self, hash: Hash) -> sc_client_api::blockchain::Result<BlockStatus> {
        self.client.lock().status(hash)
    }

    fn number(&self, hash: Hash) -> sc_client_api::blockchain::Result<Option<BlockNumber>> {
        self.client.lock().number(hash)
    }

    fn hash(&self, number: BlockNumber) -> sc_client_api::blockchain::Result<Option<Hash>> {
        self.client.lock().hash(number)
    }
}

impl BlockchainEvents<Block> for MockClient {
    fn import_notification_stream(&self) -> ImportNotifications<Block> {
        unimplemented!()
    }

    fn finality_notification_stream(&self) -> FinalityNotifications<Block> {
        self.client.lock().finality_notification_stream()
    }

    fn storage_changes_notification_stream(
        &self,
        _filter_keys: Option<&[StorageKey]>,
        _child_filter_keys: Option<&[(StorageKey, Option<Vec<StorageKey>>)]>,
    ) -> sc_client_api::blockchain::Result<StorageEventStream<Hash>> {
        unimplemented!()
    }

    fn every_import_notification_stream(&self) -> ImportNotifications<Block> {
        unimplemented!()
    }
}

impl ProvideRuntimeApi<Block> for MockClient {
    type Api = MockRuntimeApi;

    fn runtime_api(&self) -> ApiRef<'_, Self::Api> {
        MockRuntimeApi {
            data: self.runtime_api_params.clone(),
        }
        .into()
    }
}

sp_api::mock_impl_runtime_apis! {
    impl HyperdriveApi<Block, MmrHash> for MockRuntimeApi {
        fn number_of_leaves(&self, _instance: HyperdriveInstance) -> LeafIndex {
            self.data.lock().num_leaves
        }

        fn first_mmr_block_number(_instance: HyperdriveInstance) -> Option<NumberFor<Block>> {
            Some(0u64)
        }

        fn leaf_meta(_instance: HyperdriveInstance, leaf_index: LeafIndex) -> Option<(<Block as BlockT>::Hash, MmrHash)> {
            self.data.lock().leaf_meta.get(&leaf_index).copied()
        }

        fn last_message_excl_by_block(_instance: HyperdriveInstance, block_number: NumberFor<Block>) -> Option<LeafIndex> {
            self.data.lock().last_message_excl_by_block.get(&block_number).copied()
        }

        fn snapshot_roots(_instance: HyperdriveInstance, _next_expected_snapshot_number: SnapshotNumber) -> Result<Vec<(SnapshotNumber, <Block as BlockT>::Hash)>, MMRError> {
            Err(MMRError::GenerateProof)
        }

        fn snapshot_root(_instance: HyperdriveInstance, _next_expected_snapshot_number: SnapshotNumber) -> Result<Option<(SnapshotNumber, <Block as BlockT>::Hash)>, MMRError> {
            Err(MMRError::GenerateProof)
        }

        fn generate_target_chain_proof(
            _instance: HyperdriveInstance,
            _next_message_number: LeafIndex,
            _maximum_messages: Option<u64>,
            _latest_known_snapshot_number: SnapshotNumber,
        ) -> Result<Option<TargetChainProof<MmrHash>>, MMRError> {
            Err(MMRError::GenerateProof)
        }
    }
}

pub(crate) fn run_test_with_mmr_gadget<F, Fut>(post_gadget: F)
where
    F: FnOnce(Arc<MockClient>) -> Fut + 'static,
    Fut: Future<Output = ()>,
{
    run_test_with_mmr_gadget_pre_post(|_| async {}, post_gadget);
}

pub(crate) fn run_test_with_mmr_gadget_pre_post<F, G, RetF, RetG>(pre_gadget: F, post_gadget: G)
where
    F: FnOnce(Arc<MockClient>) -> RetF + 'static,
    G: FnOnce(Arc<MockClient>) -> RetG + 'static,
    RetF: Future<Output = ()>,
    RetG: Future<Output = ()>,
{
    let client = Arc::new(MockClient::new());
    run_test_with_mmr_gadget_pre_post_using_client(client, pre_gadget, post_gadget)
}

pub(crate) fn run_test_with_mmr_gadget_pre_post_using_client<F, G, RetF, RetG>(
    client: Arc<MockClient>,
    pre_gadget: F,
    post_gadget: G,
) where
    F: FnOnce(Arc<MockClient>) -> RetF + 'static,
    G: FnOnce(Arc<MockClient>) -> RetG + 'static,
    RetF: Future<Output = ()>,
    RetG: Future<Output = ()>,
{
    let client_clone = client.clone();
    let runtime = Runtime::new().unwrap();
    runtime.block_on(async move { pre_gadget(client_clone).await });

    let client_clone = client.clone();
    runtime.spawn(async move {
        let backend = client_clone.backend.clone();
        MmrGadget::<TezosInstance, _, _, _, _>::start(
            client_clone,
            backend,
            MockRuntimeApi::INDEXING_PREFIX.to_vec(),
            MockRuntimeApi::TEMP_INDEXING_PREFIX.to_vec(),
        )
        .await
    });

    runtime.block_on(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;

        post_gadget(client).await
    });
}
