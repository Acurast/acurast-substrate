//! Node-specific RPC methods for interaction with pallet-acurast-marketplace.

use std::{marker::PhantomData, sync::Arc};

use crate::{JobAssignment, MarketplaceRuntimeApi, PartialJobRegistration, RuntimeApiError};
use codec::Codec;
use frame_support::sp_runtime::traits::{Block as BlockT, HashingFor, MaybeSerializeDeserialize};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    types::error::{CallError, ErrorObject},
};
use pallet_acurast::{Attestation, Environment, JobId, MultiOrigin, ParameterBound};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;

const RUNTIME_ERROR: i32 = 8001;
const MARKETPLACE_ERROR: i32 = 8011;

#[rpc(client, server)]
pub trait MarketplaceApi<
    BlockHash,
    Reward: MaybeSerializeDeserialize,
    AccountId: MaybeSerializeDeserialize,
    Extra: MaybeSerializeDeserialize,
    MaxAllowedSources: ParameterBound,
    MaxEnvVars: ParameterBound,
    EnvKeyMaxSize: ParameterBound,
    EnvValueMaxSize: ParameterBound,
>
{
    /// Filters the given `sources` by those recently seen and matching partially specified `registration`
    /// and whitelisting `consumer` if specifying a whitelist.
    #[method(name = "filterMatchingSources")]
    fn filter_matching_sources(
        &self,
        registration: PartialJobRegistration<Reward, AccountId, MaxAllowedSources>,
        sources: Vec<AccountId>,
        consumer: Option<MultiOrigin<AccountId>>,
        latest_seen_after: Option<u128>,
    ) -> RpcResult<Vec<AccountId>>;

    /// Retrieves the job environment.
    #[method(name = "orchestrator_jobEnvironment")]
    fn job_environment(
        &self,
        job_id: JobId<AccountId>,
        source: AccountId,
    ) -> RpcResult<Option<Environment<MaxEnvVars, EnvKeyMaxSize, EnvValueMaxSize>>>;

    /// Retrieves the job assignment with the aggregated job.
    #[method(name = "orchestrator_matchedJobs")]
    fn matched_jobs(
        &self,
        source: AccountId,
    ) -> RpcResult<Vec<JobAssignment<Reward, AccountId, MaxAllowedSources, Extra>>>;

    /// Retrieves a processor's attestation.
    #[method(name = "orchestrator_attestation")]
    fn attestation(&self, source: AccountId) -> RpcResult<Option<Attestation>>;

    /// Retrieves a processor's attestation.
    #[method(name = "orchestrator_is_attested")]
    fn is_attested(&self, source: AccountId) -> RpcResult<bool>;
}

/// RPC methods.
pub struct Marketplace<Client, B> {
    client: Arc<Client>,
    _marker: PhantomData<B>,
}

impl<C, B> Marketplace<C, B> {
    /// Create new `Marketplace` with the given reference to the client.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

#[async_trait]
impl<
        Client,
        Block,
        Reward,
        AccountId,
        Extra,
        MaxAllowedSources,
        MaxEnvVars,
        EnvKeyMaxSize,
        EnvValueMaxSize,
    >
    MarketplaceApiServer<
        HashingFor<Block>,
        Reward,
        AccountId,
        Extra,
        MaxAllowedSources,
        MaxEnvVars,
        EnvKeyMaxSize,
        EnvValueMaxSize,
    > for Marketplace<Client, Block>
where
    Block: BlockT,
    Client: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    Client::Api: MarketplaceRuntimeApi<
        Block,
        Reward,
        AccountId,
        Extra,
        MaxAllowedSources,
        MaxEnvVars,
        EnvKeyMaxSize,
        EnvValueMaxSize,
    >,
    Reward: MaybeSerializeDeserialize + Codec + Send + Sync + 'static,
    AccountId: MaybeSerializeDeserialize + Codec + Send + Sync + 'static,
    Extra: MaybeSerializeDeserialize + Codec + Send + Sync + 'static,
    MaxAllowedSources: ParameterBound,
    MaxEnvVars: ParameterBound,
    EnvKeyMaxSize: ParameterBound,
    EnvValueMaxSize: ParameterBound,
{
    fn filter_matching_sources(
        &self,
        registration: PartialJobRegistration<Reward, AccountId, MaxAllowedSources>,
        sources: Vec<AccountId>,
        consumer: Option<MultiOrigin<AccountId>>,
        latest_seen_after: Option<u128>,
    ) -> RpcResult<Vec<AccountId>> {
        let api = self.client.runtime_api();
        let roots = api
            .filter_matching_sources(
                self.client.info().best_hash,
                registration,
                sources,
                consumer,
                latest_seen_after,
            )
            .map_err(runtime_error_into_rpc_error)?
            .map_err(marketplace_error_into_rpc_error)?;
        Ok(roots)
    }

    fn job_environment(
        &self,
        job_id: JobId<AccountId>,
        source: AccountId,
    ) -> RpcResult<Option<Environment<MaxEnvVars, EnvKeyMaxSize, EnvValueMaxSize>>> {
        let api = self.client.runtime_api();
        let env = api
            .job_environment(self.client.info().best_hash, job_id, source)
            .map_err(runtime_error_into_rpc_error)?
            .map_err(marketplace_error_into_rpc_error)?;
        Ok(env)
    }

    fn matched_jobs(
        &self,
        source: AccountId,
    ) -> RpcResult<Vec<JobAssignment<Reward, AccountId, MaxAllowedSources, Extra>>> {
        let api = self.client.runtime_api();
        let jobs = api
            .matched_jobs(self.client.info().best_hash, source)
            .map_err(runtime_error_into_rpc_error)?
            .map_err(marketplace_error_into_rpc_error)?;
        Ok(jobs)
    }

    fn attestation(&self, source: AccountId) -> RpcResult<Option<Attestation>> {
        let api = self.client.runtime_api();
        let attestation = api
            .attestation(self.client.info().best_hash, source)
            .map_err(runtime_error_into_rpc_error)?
            .map_err(marketplace_error_into_rpc_error)?;
        Ok(attestation)
    }

    fn is_attested(&self, source: AccountId) -> RpcResult<bool> {
        Ok(self.attestation(source)?.is_some())
    }
}

/// Converts an marketplace-specific error into a [`CallError`].
fn marketplace_error_into_rpc_error(err: RuntimeApiError) -> CallError {
    let error_code = MARKETPLACE_ERROR
        + match err {
            RuntimeApiError::FilterMatchingSources => 1,
            RuntimeApiError::MatchedJobs => 3,
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
