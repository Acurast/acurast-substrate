// This file is part of Substrate.

// Copyright (C) 2021-2022 Parity Technologies (UK) Ltd.
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

//! Node-specific RPC methods for interaction with pallet-acurast-hyperdrive-outgoing.

use std::{marker::PhantomData, sync::Arc};

use codec::Codec;
use jsonrpsee::{
    core::{async_trait, RpcResult},
    types::error::{CallError, ErrorObject},
};
use pallet_acurast_hyperdrive::instances::HyperdriveInstanceName;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block as BlockT;
use sp_runtime::traits::{HashingFor, MaybeSerializeDeserialize};

use crate::{HyperdriveApi, LeafIndex, MMRError, SnapshotNumber, TargetChainProof};

const RUNTIME_ERROR: i32 = 8000;
const MMR_ERROR: i32 = 8010;

pub trait RpcInstance: Send + Sync {
    /// Name of the `hyperdrive_outgoing_<target chain>_snapshotRoots` RPC.
    const SNAPSHOT_ROOTS: &'static str;
    /// Name of the `hyperdrive_outgoing_<target chain>_snapshotRoot` RPC.
    const SNAPSHOT_ROOT: &'static str;
    /// Name of the `hyperdrive_outgoing_<target chain>_generateProof` RPC.
    const GENERATE_PROOF: &'static str;
}

/// Hyperdrive RPC methods.
///
/// The following is the expansion of the following macro code, adapted to take an `I: RpcInstance` to dynmically replace the `"hyperdrive_outgoing_CHAIN"` namespace prefix from RPC methods.
/// ```rust
/// use jsonrpsee::{
///     core::RpcResult,
///     proc_macros::rpc,
/// };
/// use sp_runtime::traits::MaybeSerializeDeserialize;
///
/// use pallet_acurast_hyperdrive_outgoing::{LeafIndex, SnapshotNumber, TargetChainProof};
///
/// #[rpc(client, server, namespace="hyperdrive_outgoing_CHAIN")]
/// pub trait MmrApi<BlockHash, MmrHash: MaybeSerializeDeserialize> {
///     /// Returns the snapshot MMR roots from `next_expected_snapshot_number, ...` onwards or an empty vec if no new snapshots.
///     #[method(name = "snapshotRoots")]
///     fn snapshot_roots(
///         &self,
///         next_expected_snapshot_number: SnapshotNumber,
///     ) -> RpcResult<Vec<(SnapshotNumber, MmrHash)>>;
///
///     /// Returns the snapshot MMR root `next_expected_snapshot_number` or None if not snapshot not yet taken.
///     #[method(name = "snapshotRoot")]
///     fn snapshot_root(
///         &self,
///         next_expected_snapshot_number: SnapshotNumber,
///     ) -> RpcResult<Option<(SnapshotNumber, MmrHash)>>;
///
///     /// Generates a self-contained MMR proof for the messages in the range `[next_message_number..last_message_excl]`.
///     /// Leaves with their leaf index and position are part of the proof structure and contain the message encoded for the target chain.
///     ///
///     /// This rpc calls into the runtime function [`crate::Pallet::generate_target_chain_proof`].
///     /// Optionally via `at`, a block hash at which the runtime should be queried can be specified.
///     #[method(name = "generateProof")]
///     fn generate_target_chain_proof(
///         &self,
///         next_message_number: LeafIndex,
///         maximum_messages: Option<u64>,
///         latest_known_snapshot_number: SnapshotNumber,
///     ) -> RpcResult<Option<TargetChainProof<MmrHash>>>;
/// }
/// ```
#[jsonrpsee::core::__reexports::async_trait]
#[doc = "Server trait implementation for the `MmrApi` RPC API."]
pub trait MmrApiServer<I: RpcInstance, BlockHash, MmrHash: MaybeSerializeDeserialize>:
    Sized + Send + Sync + 'static
{
    #[doc = " Returns the snapshot MMR roots from `next_expected_snapshot_number, ...` onwards or an empty vec if no new snapshots."]
    fn snapshot_roots(
        &self,
        next_expected_snapshot_number: SnapshotNumber,
    ) -> RpcResult<Vec<(SnapshotNumber, MmrHash)>>;

    #[doc = " Returns the snapshot MMR root `next_expected_snapshot_number` or None if not snapshot not yet taken."]
    fn snapshot_root(
        &self,
        next_expected_snapshot_number: SnapshotNumber,
    ) -> RpcResult<Option<(SnapshotNumber, MmrHash)>>;

    #[doc = " Generates a self-contained MMR proof for the messages in the range `[next_message_number..last_message_excl]`."]
    #[doc = " Leaves with their leaf index and position are part of the proof structure and contain the message encoded for the target chain."]
    #[doc = ""]
    #[doc = " This rpc calls into the runtime function [`crate::Pallet::generate_target_chain_proof`]."]
    #[doc = " Optionally via `at`, a block hash at which the runtime should be queried can be specified."]
    fn generate_target_chain_proof(
        &self,
        next_message_number: LeafIndex,
        maximum_messages: Option<u64>,
        latest_known_snapshot_number: SnapshotNumber,
    ) -> RpcResult<Option<TargetChainProof<MmrHash>>>;
    #[doc = "Collects all the methods and subscriptions defined in the trait and adds them into a single `RpcModule`."]
    fn into_rpc(self) -> jsonrpsee::RpcModule<Self>
    where
        BlockHash: Send + Sync + 'static,
        MmrHash: Send + Sync + 'static + jsonrpsee::core::Serialize,
    {
        let mut rpc = jsonrpsee::RpcModule::new(self);
        {
            let res = rpc.register_method(I::SNAPSHOT_ROOTS, |params, context| {
                let next_expected_snapshot_number = if params.is_object() {
                    #[derive(jsonrpsee::core::__reexports::serde::Deserialize)]
                    #[serde(crate = "jsonrpsee :: core :: __reexports :: serde")]
                    struct ParamsObject<G0> {
                        #[serde(
                            alias = "next_expected_snapshot_number",
                            alias = "nextExpectedSnapshotNumber"
                        )]
                        next_expected_snapshot_number: G0,
                    }
                    let parsed: ParamsObject<SnapshotNumber> = params.parse().map_err(|e| {
                        jsonrpsee::tracing::error!(
                            "Failed to parse JSON-RPC params as object: {}",
                            e
                        );
                        e
                    })?;
                    parsed.next_expected_snapshot_number
                } else {
                    let mut seq = params.sequence();
                    let next_expected_snapshot_number: SnapshotNumber = match seq.next() {
                        Ok(v) => v,
                        Err(e) => {
                            jsonrpsee::tracing::error!(
                                concat!(
                                    "Error parsing \"",
                                    stringify!(next_expected_snapshot_number),
                                    "\" as \"",
                                    stringify!(SnapshotNumber),
                                    "\": {:?}"
                                ),
                                e
                            );
                            return Err(e.into());
                        }
                    };
                    next_expected_snapshot_number
                };
                context.snapshot_roots(next_expected_snapshot_number)
            });
            debug_assert!(
                res.is_ok(),
                "RPC macro method names should never conflict, this is a bug, please report it."
            );
        }
        {
            let res = rpc.register_method(I::SNAPSHOT_ROOT, |params, context| {
                let next_expected_snapshot_number = if params.is_object() {
                    #[derive(jsonrpsee::core::__reexports::serde::Deserialize)]
                    #[serde(crate = "jsonrpsee :: core :: __reexports :: serde")]
                    struct ParamsObject<G0> {
                        #[serde(
                            alias = "next_expected_snapshot_number",
                            alias = "nextExpectedSnapshotNumber"
                        )]
                        next_expected_snapshot_number: G0,
                    }
                    let parsed: ParamsObject<SnapshotNumber> = params.parse().map_err(|e| {
                        jsonrpsee::tracing::error!(
                            "Failed to parse JSON-RPC params as object: {}",
                            e
                        );
                        e
                    })?;
                    parsed.next_expected_snapshot_number
                } else {
                    let mut seq = params.sequence();
                    let next_expected_snapshot_number: SnapshotNumber = match seq.next() {
                        Ok(v) => v,
                        Err(e) => {
                            jsonrpsee::tracing::error!(
                                concat!(
                                    "Error parsing \"",
                                    stringify!(next_expected_snapshot_number),
                                    "\" as \"",
                                    stringify!(SnapshotNumber),
                                    "\": {:?}"
                                ),
                                e
                            );
                            return Err(e.into());
                        }
                    };
                    next_expected_snapshot_number
                };
                context.snapshot_root(next_expected_snapshot_number)
            });
            debug_assert!(
                res.is_ok(),
                "RPC macro method names should never conflict, this is a bug, please report it."
            );
        }
        {
            let res = rpc.register_method(I::GENERATE_PROOF, |params, context| {
                let (next_message_number, maximum_messages, latest_known_snapshot_number) =
                    if params.is_object() {
                        #[derive(jsonrpsee::core::__reexports::serde::Deserialize)]
                        #[serde(crate = "jsonrpsee :: core :: __reexports :: serde")]
                        struct ParamsObject<G0, G1, G2> {
                            #[serde(alias = "next_message_number", alias = "nextMessageNumber")]
                            next_message_number: G0,
                            #[serde(alias = "maximum_messages", alias = "maximumMessages")]
                            maximum_messages: G1,
                            #[serde(
                                alias = "latest_known_snapshot_number",
                                alias = "latestKnownSnapshotNumber"
                            )]
                            latest_known_snapshot_number: G2,
                        }
                        let parsed: ParamsObject<LeafIndex, Option<u64>, SnapshotNumber> =
                            params.parse().map_err(|e| {
                                jsonrpsee::tracing::error!(
                                    "Failed to parse JSON-RPC params as object: {}",
                                    e
                                );
                                e
                            })?;
                        (
                            parsed.next_message_number,
                            parsed.maximum_messages,
                            parsed.latest_known_snapshot_number,
                        )
                    } else {
                        let mut seq = params.sequence();
                        let next_message_number: LeafIndex = match seq.next() {
                            Ok(v) => v,
                            Err(e) => {
                                jsonrpsee::tracing::error!(
                                    concat!(
                                        "Error parsing \"",
                                        stringify!(next_message_number),
                                        "\" as \"",
                                        stringify!(LeafIndex),
                                        "\": {:?}"
                                    ),
                                    e
                                );
                                return Err(e.into());
                            }
                        };
                        let maximum_messages: Option<u64> = match seq.optional_next() {
                            Ok(v) => v,
                            Err(e) => {
                                jsonrpsee::tracing::error!(
                                    concat!(
                                        "Error parsing optional \"",
                                        stringify!(maximum_messages),
                                        "\" as \"",
                                        stringify!(Option<u64>),
                                        "\": {:?}"
                                    ),
                                    e
                                );
                                return Err(e.into());
                            }
                        };
                        let latest_known_snapshot_number: SnapshotNumber = match seq.next() {
                            Ok(v) => v,
                            Err(e) => {
                                jsonrpsee::tracing::error!(
                                    concat!(
                                        "Error parsing \"",
                                        stringify!(latest_known_snapshot_number),
                                        "\" as \"",
                                        stringify!(SnapshotNumber),
                                        "\": {:?}"
                                    ),
                                    e
                                );
                                return Err(e.into());
                            }
                        };
                        (
                            next_message_number,
                            maximum_messages,
                            latest_known_snapshot_number,
                        )
                    };
                context.generate_target_chain_proof(
                    next_message_number,
                    maximum_messages,
                    latest_known_snapshot_number,
                )
            });
            debug_assert!(
                res.is_ok(),
                "RPC macro method names should never conflict, this is a bug, please report it."
            );
        }
        rpc
    }
}
#[jsonrpsee::core::__reexports::async_trait]
#[doc = "Client implementation for the `MmrApi` RPC API."]
pub trait MmrApiClient<I: RpcInstance, BlockHash, MmrHash: MaybeSerializeDeserialize>:
    jsonrpsee::core::client::ClientT
where
    BlockHash: Send + Sync + 'static,
    MmrHash: Send + Sync + 'static + jsonrpsee::core::DeserializeOwned,
{
    #[doc = " Returns the snapshot MMR roots from `next_expected_snapshot_number, ...` onwards or an empty vec if no new snapshots."]
    async fn snapshot_roots(
        &self,
        next_expected_snapshot_number: SnapshotNumber,
    ) -> RpcResult<Vec<(SnapshotNumber, MmrHash)>> {
        let params = {
            {
                let mut params = jsonrpsee::core::params::ArrayParams::new();
                if let Err(err) = params.insert(next_expected_snapshot_number) {
                    panic!(
                        "Parameter `{}` cannot be serialized: {:?}",
                        stringify!(next_expected_snapshot_number),
                        err
                    );
                }
                params
            }
        };
        self.request(I::SNAPSHOT_ROOTS, params).await
    }
    #[doc = " Returns the snapshot MMR root `next_expected_snapshot_number` or None if not snapshot not yet taken."]
    async fn snapshot_root(
        &self,
        next_expected_snapshot_number: SnapshotNumber,
    ) -> RpcResult<Option<(SnapshotNumber, MmrHash)>> {
        let params = {
            {
                let mut params = jsonrpsee::core::params::ArrayParams::new();
                if let Err(err) = params.insert(next_expected_snapshot_number) {
                    panic!(
                        "Parameter `{}` cannot be serialized: {:?}",
                        stringify!(next_expected_snapshot_number),
                        err
                    );
                }
                params
            }
        };
        self.request(I::SNAPSHOT_ROOT, params).await
    }
    #[doc = " Generates a self-contained MMR proof for the messages in the range `[next_message_number..last_message_excl]`."]
    #[doc = " Leaves with their leaf index and position are part of the proof structure and contain the message encoded for the target chain."]
    #[doc = ""]
    #[doc = " This rpc calls into the runtime function [`crate::Pallet::generate_target_chain_proof`]."]
    #[doc = " Optionally via `at`, a block hash at which the runtime should be queried can be specified."]
    async fn generate_target_chain_proof(
        &self,
        next_message_number: LeafIndex,
        maximum_messages: Option<u64>,
        latest_known_snapshot_number: SnapshotNumber,
    ) -> RpcResult<Option<TargetChainProof<MmrHash>>> {
        let params = {
            {
                let mut params = jsonrpsee::core::params::ArrayParams::new();
                if let Err(err) = params.insert(next_message_number) {
                    panic!(
                        "Parameter `{}` cannot be serialized: {:?}",
                        stringify!(next_message_number),
                        err
                    );
                }
                if let Err(err) = params.insert(maximum_messages) {
                    panic!(
                        "Parameter `{}` cannot be serialized: {:?}",
                        stringify!(maximum_messages),
                        err
                    );
                }
                if let Err(err) = params.insert(latest_known_snapshot_number) {
                    panic!(
                        "Parameter `{}` cannot be serialized: {:?}",
                        stringify!(latest_known_snapshot_number),
                        err
                    );
                }
                params
            }
        };
        self.request(I::GENERATE_PROOF, params).await
    }
}
impl<I, TypeJsonRpseeInteral, BlockHash, MmrHash: MaybeSerializeDeserialize>
    MmrApiClient<I, BlockHash, MmrHash> for TypeJsonRpseeInteral
where
    I: RpcInstance,
    TypeJsonRpseeInteral: jsonrpsee::core::client::ClientT,
    BlockHash: Send + Sync + 'static,
    MmrHash: Send + Sync + 'static + jsonrpsee::core::DeserializeOwned,
{
}

/// MMR RPC methods.
pub struct Mmr<I, Client, Block> {
    client: Arc<Client>,
    _marker: PhantomData<(I, Block)>,
}

impl<I, C, B> Mmr<I, C, B> {
    /// Create new `Mmr` with the given reference to the client.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

#[async_trait]
impl<I: RpcInstance + HyperdriveInstanceName + 'static, Client, Block, MmrHash>
    MmrApiServer<I, HashingFor<Block>, MmrHash> for Mmr<I, Client, (Block, MmrHash)>
where
    Block: BlockT,
    Client: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    Client::Api: HyperdriveApi<Block, MmrHash>,
    MmrHash: MaybeSerializeDeserialize + Codec + Send + Sync + 'static,
{
    fn snapshot_roots(
        &self,
        next_expected_snapshot_number: SnapshotNumber,
    ) -> RpcResult<Vec<(SnapshotNumber, MmrHash)>> {
        let api = self.client.runtime_api();
        let roots = api
            .snapshot_roots(
                self.client.info().best_hash,
                I::NAME,
                next_expected_snapshot_number,
            )
            .map_err(runtime_error_into_rpc_error)?
            .map_err(mmr_error_into_rpc_error)?;
        Ok(roots)
    }

    fn snapshot_root(
        &self,
        next_expected_snapshot_number: SnapshotNumber,
    ) -> RpcResult<Option<(SnapshotNumber, MmrHash)>> {
        let api = self.client.runtime_api();
        let root = api
            .snapshot_root(
                self.client.info().best_hash,
                I::NAME,
                next_expected_snapshot_number,
            )
            .map_err(runtime_error_into_rpc_error)?
            .map_err(mmr_error_into_rpc_error)?;
        Ok(root)
    }

    fn generate_target_chain_proof(
        &self,
        next_message_number: LeafIndex,
        maximum_messages: Option<u64>,
        latest_known_snapshot_number: SnapshotNumber,
    ) -> RpcResult<Option<TargetChainProof<MmrHash>>> {
        let api = self.client.runtime_api();

        let proof = api
            .generate_target_chain_proof(
                self.client.info().best_hash,
                I::NAME,
                next_message_number,
                maximum_messages,
                latest_known_snapshot_number,
            )
            .map_err(runtime_error_into_rpc_error)?
            .map_err(mmr_error_into_rpc_error)?;

        Ok(proof)
    }
}

/// Converts an mmr-specific error into a [`CallError`].
fn mmr_error_into_rpc_error(err: MMRError) -> CallError {
    let error_code = MMR_ERROR
        + match err {
            MMRError::Push => 1,
            MMRError::GetRoot => 2,
            MMRError::Commit => 3,
            MMRError::GenerateProof => 4,
            MMRError::GenerateProofNoSnapshot => 5,
            MMRError::GenerateProofFutureSnapshot => 6,
            MMRError::GenerateProofFutureMessage => 7,
            MMRError::Verify => 8,
            MMRError::LeafNotFound => 9,
            MMRError::InconsistentSnapshotMeta => 10,
        };

    CallError::Custom(ErrorObject::owned(
        error_code,
        err.to_string(),
        Some(format!("{:?}", err)),
    ))
}

/// Converts a runtime trap into a [`CallError`].
fn runtime_error_into_rpc_error(err: impl std::fmt::Debug) -> CallError {
    CallError::Custom(ErrorObject::owned(
        RUNTIME_ERROR,
        "Runtime trapped",
        Some(format!("{:?}", err)),
    ))
}
