// Copyright 2019-2022 PureStake Inc.
// Copyright 2023 Papers AG.
// This file is adapted from Moonbeam.

// Moonbeam is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Moonbeam is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Moonbeam.  If not, see <http://www.gnu.org/licenses/>.

use std::sync::Arc;

use acurast_rococo_runtime::pallet_acurast_marketplace;
use sc_client_api::{Backend as BackendT, BlockchainEvents, KeysIter, MerkleValue, PairsIter};
use sp_api::{CallApiAt, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_consensus::BlockStatus;
use sp_core::H256;
use sp_runtime::{
	generic::SignedBlock,
	traits::{BlakeTwo256, Block as BlockT, NumberFor},
	Justifications,
};
use sp_storage::{ChildInfo, StorageData, StorageKey};

use acurast_runtime_common::AuraId;

use acurast_runtime_common::{
	opaque::{Block, Header},
	EnvKeyMaxSize, EnvValueMaxSize, MaxAllowedSources, MaxEnvVars, MaxSlots,
};
pub use acurast_runtime_common::{AccountId, Balance, BlockNumber, Hash, Nonce};
use pallet_acurast_marketplace::RegistrationExtra;

use crate::service::{self, ParachainBackend, ParachainClient};

/// A set of APIs that polkadot-like runtimes must implement.
///
/// This trait has no methods or associated type. It is a concise marker for all the trait bounds
/// that it contains.
pub trait RuntimeApiCollection:
	sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
	+ sp_api::ApiExt<Block>
	+ sp_block_builder::BlockBuilder<Block>
	+ substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>
	+ pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance>
	+ sp_api::Metadata<Block>
	+ sp_offchain::OffchainWorkerApi<Block>
	+ sp_session::SessionKeys<Block>
	+ pallet_acurast_hyperdrive_outgoing::HyperdriveApi<Block, H256>
	+ pallet_acurast_marketplace::MarketplaceRuntimeApi<
		Block,
		Balance,
		AccountId,
		RegistrationExtra<Balance, AccountId, MaxSlots>,
		MaxAllowedSources,
		MaxEnvVars,
		EnvKeyMaxSize,
		EnvValueMaxSize,
	> + sp_consensus_aura::AuraApi<Block, AuraId>
	+ cumulus_primitives_core::CollectCollationInfo<Block>
{
}

impl<Api> RuntimeApiCollection for Api where
	Api: sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
		+ sp_api::ApiExt<Block>
		+ sp_block_builder::BlockBuilder<Block>
		+ substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>
		+ pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance>
		+ sp_api::Metadata<Block>
		+ sp_offchain::OffchainWorkerApi<Block>
		+ sp_session::SessionKeys<Block>
		+ pallet_acurast_hyperdrive_outgoing::HyperdriveApi<Block, H256>
		+ pallet_acurast_marketplace::MarketplaceRuntimeApi<
			Block,
			Balance,
			AccountId,
			RegistrationExtra<Balance, AccountId, MaxSlots>,
			MaxAllowedSources,
			MaxEnvVars,
			EnvKeyMaxSize,
			EnvValueMaxSize,
		> + sp_consensus_aura::AuraApi<Block, AuraId>
		+ cumulus_primitives_core::CollectCollationInfo<Block>
{
}

/// Config that abstracts over all available client implementations.
///
/// For a concrete type there exists [`ClientVariant`].
pub trait AbstractClient<Block, Backend>:
	BlockchainEvents<Block>
	+ Sized
	+ Send
	+ Sync
	+ ProvideRuntimeApi<Block>
	+ HeaderBackend<Block>
	+ CallApiAt<Block, StateBackend = Backend::State>
where
	Block: BlockT,
	Backend: BackendT<Block>,
	Backend::State: sc_client_api::backend::StateBackend<BlakeTwo256>,
{
}

impl<Block, Backend, Client> AbstractClient<Block, Backend> for Client
where
	Block: BlockT,
	Backend: BackendT<Block>,
	Backend::State: sc_client_api::backend::StateBackend<BlakeTwo256>,
	Client: BlockchainEvents<Block>
		+ ProvideRuntimeApi<Block>
		+ HeaderBackend<Block>
		+ Sized
		+ Send
		+ Sync
		+ CallApiAt<Block, StateBackend = Backend::State>,
{
}

/// Execute something with the client instance.
///
/// As there exist multiple chains inside Acurast, like Acurast Rococo, Acurast Kusama etc.,
/// there can exist different kinds of client types. As these
/// client types differ in the generics that are being used, we can not easily
/// return them from a function. For returning them from a function there exists
/// [`ClientVariant`]. However, the problem on how to use this client instance still
/// exists. This trait "solves" it in a dirty way. It requires a type to
/// implement this trait and then the [`execute_with_client`](ExecuteWithClient:
/// :execute_with_client) function can be called with any possible client
/// instance.
///
/// In a perfect world, we could make a closure work in this way.
pub trait ExecuteWithClient {
	/// The return type when calling this instance.
	type Output;

	/// Execute whatever should be executed with the given client instance.
	fn execute_with_client<Client, Api, Backend>(self, client: Arc<Client>) -> Self::Output
	where
		Backend: sc_client_api::Backend<Block>,
		Backend::State: sc_client_api::backend::StateBackend<BlakeTwo256>,
		Client: AbstractClient<Block, Backend, Api = Api> + 'static;
}

/// A handle to a Acurast client instance.
///
/// The Acurast service supports multiple different runtimes (Acurast Rococo, Acurast Kusama, etc.)
/// As each runtime has a specialized client, we need to hide them
/// behind a trait. This is this trait.
///
/// When wanting to work with the inner client, you need to use `execute_with`.
pub trait ClientHandle {
	/// Execute the given something with the client.
	fn execute_with<T: ExecuteWithClient>(&self, t: T) -> T::Output;
}

/// The exhaustive enum of client for each [`service::NetworkVariant`].
#[derive(Clone)]
pub enum ClientVariant {
	#[cfg(any(feature = "acurast-local", feature = "acurast-dev", feature = "acurast-rococo"))]
	Testnet(
		Arc<ParachainClient<acurast_rococo_runtime::RuntimeApi, service::testnet::AcurastExecutor>>,
	),
	#[cfg(feature = "acurast-kusama")]
	Canary(
		Arc<ParachainClient<acurast_kusama_runtime::RuntimeApi, service::canary::AcurastExecutor>>,
	),
}

#[cfg(any(feature = "acurast-local", feature = "acurast-dev", feature = "acurast-rococo"))]
impl
	From<
		Arc<ParachainClient<acurast_rococo_runtime::RuntimeApi, service::testnet::AcurastExecutor>>,
	> for ClientVariant
{
	fn from(
		client: Arc<
			ParachainClient<acurast_rococo_runtime::RuntimeApi, service::testnet::AcurastExecutor>,
		>,
	) -> Self {
		Self::Testnet(client)
	}
}

#[cfg(feature = "acurast-kusama")]
impl
	From<Arc<ParachainClient<acurast_kusama_runtime::RuntimeApi, service::canary::AcurastExecutor>>>
	for ClientVariant
{
	fn from(
		client: Arc<
			ParachainClient<acurast_kusama_runtime::RuntimeApi, service::canary::AcurastExecutor>,
		>,
	) -> Self {
		Self::Canary(client)
	}
}

impl ClientHandle for ClientVariant {
	fn execute_with<T: ExecuteWithClient>(&self, t: T) -> T::Output {
		match self {
			#[cfg(any(
				feature = "acurast-local",
				feature = "acurast-dev",
				feature = "acurast-rococo"
			))]
			Self::Testnet(client) => T::execute_with_client::<_, _, ParachainBackend>(t, client.clone()),
			#[cfg(feature = "acurast-kusama")]
			Self::Canary(client) => T::execute_with_client::<_, _, ParachainBackend>(t, client.clone()),
		}
	}
}

macro_rules! match_client {
	($self:ident, $method:ident($($param:ident),*)) => {
		match $self {
			#[cfg(any(feature = "acurast-local", feature = "acurast-dev", feature = "acurast-rococo"))]
			Self::Testnet(client) => client.$method($($param),*),
			#[cfg(feature = "acurast-kusama")]
			Self::Canary(client) => client.$method($($param),*),
		}
	};
}

impl sc_client_api::UsageProvider<Block> for ClientVariant {
	fn usage_info(&self) -> sc_client_api::ClientInfo<Block> {
		match_client!(self, usage_info())
	}
}

impl sc_client_api::BlockBackend<Block> for ClientVariant {
	fn block_body(
		&self,
		hash: <Block as BlockT>::Hash,
	) -> sp_blockchain::Result<Option<Vec<<Block as BlockT>::Extrinsic>>> {
		match_client!(self, block_body(hash))
	}

	fn block_indexed_body(
		&self,
		hash: <Block as BlockT>::Hash,
	) -> sp_blockchain::Result<Option<Vec<Vec<u8>>>> {
		match_client!(self, block_indexed_body(hash))
	}

	fn block(
		&self,
		hash: <Block as BlockT>::Hash,
	) -> sp_blockchain::Result<Option<SignedBlock<Block>>> {
		match_client!(self, block(hash))
	}

	fn block_status(&self, hash: <Block as BlockT>::Hash) -> sp_blockchain::Result<BlockStatus> {
		match_client!(self, block_status(hash))
	}

	fn justifications(
		&self,
		hash: <Block as BlockT>::Hash,
	) -> sp_blockchain::Result<Option<Justifications>> {
		match_client!(self, justifications(hash))
	}

	fn block_hash(
		&self,
		number: NumberFor<Block>,
	) -> sp_blockchain::Result<Option<<Block as BlockT>::Hash>> {
		match_client!(self, block_hash(number))
	}

	fn indexed_transaction(
		&self,
		hash: <Block as BlockT>::Hash,
	) -> sp_blockchain::Result<Option<Vec<u8>>> {
		match_client!(self, indexed_transaction(hash))
	}

	fn has_indexed_transaction(
		&self,
		hash: <Block as BlockT>::Hash,
	) -> sp_blockchain::Result<bool> {
		match_client!(self, has_indexed_transaction(hash))
	}

	fn requires_full_sync(&self) -> bool {
		match_client!(self, requires_full_sync())
	}
}

impl sc_client_api::StorageProvider<Block, ParachainBackend> for ClientVariant {
	fn storage(
		&self,
		hash: <Block as BlockT>::Hash,
		key: &StorageKey,
	) -> sp_blockchain::Result<Option<StorageData>> {
		match_client!(self, storage(hash, key))
	}

	fn storage_hash(
		&self,
		hash: <Block as BlockT>::Hash,
		key: &StorageKey,
	) -> sp_blockchain::Result<Option<<Block as BlockT>::Hash>> {
		match_client!(self, storage_hash(hash, key))
	}

	fn storage_keys(
		&self,
		hash: <Block as BlockT>::Hash,
		prefix: Option<&StorageKey>,
		start_key: Option<&StorageKey>,
	) -> sp_blockchain::Result<
		KeysIter<<ParachainBackend as sc_client_api::Backend<Block>>::State, Block>,
	> {
		match_client!(self, storage_keys(hash, prefix, start_key))
	}

	fn storage_pairs(
		&self,
		hash: <Block as BlockT>::Hash,
		key_prefix: Option<&StorageKey>,
		start_key: Option<&StorageKey>,
	) -> sp_blockchain::Result<
		PairsIter<<ParachainBackend as sc_client_api::Backend<Block>>::State, Block>,
	> {
		match_client!(self, storage_pairs(hash, key_prefix, start_key))
	}

	fn child_storage(
		&self,
		hash: <Block as BlockT>::Hash,
		child_info: &ChildInfo,
		key: &StorageKey,
	) -> sp_blockchain::Result<Option<StorageData>> {
		match_client!(self, child_storage(hash, child_info, key))
	}

	fn child_storage_keys(
		&self,
		hash: <Block as BlockT>::Hash,
		child_info: ChildInfo,
		prefix: Option<&StorageKey>,
		start_key: Option<&StorageKey>,
	) -> sp_blockchain::Result<
		KeysIter<<ParachainBackend as sc_client_api::Backend<Block>>::State, Block>,
	> {
		match_client!(self, child_storage_keys(hash, child_info, prefix, start_key))
	}

	fn child_storage_hash(
		&self,
		hash: <Block as BlockT>::Hash,
		child_info: &ChildInfo,
		key: &StorageKey,
	) -> sp_blockchain::Result<Option<<Block as BlockT>::Hash>> {
		match_client!(self, child_storage_hash(hash, child_info, key))
	}

	fn closest_merkle_value(
		&self,
		hash: <Block as BlockT>::Hash,
		key: &StorageKey,
	) -> sp_blockchain::Result<Option<MerkleValue<<Block as BlockT>::Hash>>> {
		match_client!(self, closest_merkle_value(hash, key))
	}

	fn child_closest_merkle_value(
		&self,
		hash: <Block as BlockT>::Hash,
		child_info: &ChildInfo,
		key: &StorageKey,
	) -> sp_blockchain::Result<Option<MerkleValue<<Block as BlockT>::Hash>>> {
		match_client!(self, child_closest_merkle_value(hash, child_info, key))
	}
}

impl sp_blockchain::HeaderBackend<Block> for ClientVariant {
	fn header(&self, hash: Hash) -> sp_blockchain::Result<Option<Header>> {
		match_client!(self, header(hash))
	}

	fn info(&self) -> sp_blockchain::Info<Block> {
		match_client!(self, info())
	}

	fn status(&self, hash: Hash) -> sp_blockchain::Result<sp_blockchain::BlockStatus> {
		match_client!(self, status(hash))
	}

	fn number(&self, hash: Hash) -> sp_blockchain::Result<Option<BlockNumber>> {
		match_client!(self, number(hash))
	}

	fn hash(&self, number: NumberFor<Block>) -> sp_blockchain::Result<Option<Hash>> {
		match_client!(self, hash(number))
	}
}
