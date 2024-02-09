//! Node-specific RPC methods for interaction with pallet-acurast-processor-manager.

use std::{marker::PhantomData, sync::Arc};

use codec::Codec;
use frame_support::sp_runtime::traits::{Block as BlockT, HashingFor, MaybeSerializeDeserialize};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    types::error::{CallError, ErrorObject},
};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;

use crate::{ProcessorManagerRuntimeApi, RuntimeApiError, UpdateInfos};

const RUNTIME_ERROR: i32 = 8003;
const ERROR_CODE: i32 = 8011;

#[rpc(client, server)]
pub trait ProcessorManagerApi<
    BlockHash,
    AccountId: MaybeSerializeDeserialize,
    ManagerId: MaybeSerializeDeserialize,
>
{
    /// Retrieves the update infos.
    #[method(name = "getUpdateInfos")]
    fn processor_update_infos(&self, sources: AccountId) -> RpcResult<UpdateInfos>;

    /// Retrieves the manager id for a processor.
    #[method(name = "managerIdForProcessor")]
    fn manager_id_for_processor(&self, source: AccountId) -> RpcResult<ManagerId>;
}

/// RPC methods.
pub struct ProcessorManager<Client, B> {
    client: Arc<Client>,
    _marker: PhantomData<B>,
}

impl<C, B> ProcessorManager<C, B> {
    /// Create new `ProcessorManager` with the given reference to the client.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

#[async_trait]
impl<Client, Block, AccountId, ManagerId>
    ProcessorManagerApiServer<HashingFor<Block>, AccountId, ManagerId>
    for ProcessorManager<Client, (Block, AccountId, ManagerId)>
where
    Block: BlockT,
    Client: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    Client::Api: ProcessorManagerRuntimeApi<Block, AccountId, ManagerId>,
    AccountId: MaybeSerializeDeserialize + Codec + Send + Sync + 'static,
    ManagerId: MaybeSerializeDeserialize + Codec + Send + Sync + 'static,
{
    fn processor_update_infos(&self, source: AccountId) -> RpcResult<UpdateInfos> {
        let api = self.client.runtime_api();
        let env = api
            .processor_update_infos(self.client.info().best_hash, source)
            .map_err(runtime_error_into_rpc_error)?
            .map_err(error_into_rpc_error)?;
        Ok(env)
    }

    fn manager_id_for_processor(&self, source: AccountId) -> RpcResult<ManagerId> {
        let api = self.client.runtime_api();
        let manager_id = api
            .manager_id_for_processor(self.client.info().best_hash, source)
            .map_err(runtime_error_into_rpc_error)?
            .map_err(error_into_rpc_error)?;
        Ok(manager_id)
    }
}

/// Converts an marketplace-specific error into a [`CallError`].
fn error_into_rpc_error(err: RuntimeApiError) -> CallError {
    let error_code = ERROR_CODE
        + match err {
            RuntimeApiError::ProcessorUpdateInfos => 1,
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
