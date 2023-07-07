//! A collection of node-specific RPC methods.
//! Substrate provides the `sc-rpc` crate, which defines the core RPC layer
//! used by Substrate nodes. This file extends those RPC definitions with
//! capabilities that are specific to this project's runtime configuration.

#![warn(missing_docs)]

use std::sync::Arc;

use sc_client_api::AuxStore;
pub use sc_rpc::DenyUnsafe;
use sc_transaction_pool_api::TransactionPool;
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sp_core::H256;

use acurast_runtime_common::{opaque::Block, AccountId, AcurastAsset, Balance, Index as Nonce};
use pallet_acurast_hyperdrive_outgoing::{instances::tezos::TargetChainTezos, HyperdriveApi};
use pallet_acurast_marketplace::MarketplaceRuntimeApi;

/// A type representing all RPC extensions.
pub type RpcExtension = jsonrpsee::RpcModule<()>;

/// Full client dependencies
pub struct FullDeps<C, P> {
	/// The client instance to use.
	pub client: Arc<C>,
	/// Transaction pool instance.
	pub pool: Arc<P>,
	/// Whether to deny unsafe calls
	pub deny_unsafe: DenyUnsafe,
}

/// Instantiate all RPC extensions.
pub fn create_full<I, C, P>(
	deps: FullDeps<C, P>,
) -> Result<RpcExtension, Box<dyn std::error::Error + Send + Sync>>
where
	C: ProvideRuntimeApi<Block>
		+ HeaderBackend<Block>
		+ AuxStore
		+ HeaderMetadata<Block, Error = BlockChainError>
		+ Send
		+ Sync
		+ 'static,
	C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
	C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>,
	C::Api: BlockBuilder<Block>,
	/// <HB SBP Milestone Review II
	/// 
	/// Just pointing out that there is an ongoing discussion where it says that custom RPCs should be discouraged when possible.
	/// The point is that runtime api might be accessed from state_call and avoid having to release a new version of the node when the RPCs are modified.
	/// Here more context:
	/// 
	/// https://github.com/paritytech/substrate/pull/12358
	/// 
	/// No action needed here, but just wanted to point it out.
	/// >
	C::Api: HyperdriveApi<Block, H256, TargetChainTezos>,
	C::Api: MarketplaceRuntimeApi<Block, AcurastAsset, AccountId>,
	P: TransactionPool + Sync + Send + 'static,
{
	use pallet_acurast_hyperdrive_outgoing::rpc::{Mmr, MmrApiServer};
	use pallet_acurast_marketplace::rpc::{Marketplace, MarketplaceApiServer};
	use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApiServer};
	use substrate_frame_rpc_system::{System, SystemApiServer};

	let mut module = RpcExtension::new(());
	let FullDeps { client, pool, deny_unsafe } = deps;

	module.merge(System::new(client.clone(), pool, deny_unsafe).into_rpc())?;
	module.merge(TransactionPayment::new(client.clone()).into_rpc())?;
	module.merge(Mmr::<TargetChainTezos, _, _>::new(client.clone()).into_rpc())?;
	module.merge(Marketplace::<_, (Block, AcurastAsset, AccountId)>::new(client).into_rpc())?;
	Ok(module)
}
