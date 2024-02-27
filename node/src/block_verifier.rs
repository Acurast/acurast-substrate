use cumulus_client_consensus_aura::{BuildVerifierParams, InherentDataProviderExt};
use sc_client_api::HeaderBackend;
use sc_consensus::{
	import_queue::{BasicQueue, Verifier as VerifierT},
	BlockImport, BlockImportParams,
};
use sc_telemetry::TelemetryHandle;
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder as BlockBuilderApi;
use sp_blockchain::Result as ClientResult;
use sp_consensus::error::Error as ConsensusError;
pub use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_consensus_aura::AuraApi;
use sp_inherents::CreateInherentDataProviders;
use sp_runtime::{
	app_crypto::AppCrypto,
	traits::{Block as BlockT, Header as HeaderT},
};
use std::sync::Arc;

use nimbus_primitives::CompatibleDigestItem as NimbusDigestItem;
use sp_consensus_aura::digests::CompatibleDigestItem as AuraDigestItem;

const LOG: &str = "acurast-consensus-verifier";

enum ConsensusKind {
	Aura,
	Nimbus,
}

fn get_consensus_kind<Block: BlockT>(block_header: &Block::Header) -> ConsensusKind {
	// Grab the seal digest. Assume it is the last item (since it is a seal after-all).
	let mut header = block_header.clone();
	let seal = header.digest_mut().pop().expect("Block should have at least one digest on it");

	if NimbusDigestItem::as_nimbus_seal(&seal).is_some() {
		log::debug!(target: LOG, "Block sealed with Nimbus consensus");
		return ConsensusKind::Nimbus
	} else if AuraDigestItem::<<AuraId as AppCrypto>::Signature>::as_aura_seal(&seal).is_some() {
		log::debug!(target: LOG, "Block sealed with Aura consensus");
		return ConsensusKind::Aura
	}

	panic!("Block was sealed with an unknown consensus");
}

/// Verify a justification of a block
struct AgnosticBlockVerifier<Client, Block: BlockT, AuraCIDP, NimbusCIDP> {
	aura_verifier: cumulus_client_consensus_aura::AuraVerifier<
		Client,
		<AuraId as AppCrypto>::Pair,
		AuraCIDP,
		<<Block as BlockT>::Header as HeaderT>::Number,
	>,
	nimbus_verifier: nimbus_consensus::Verifier<Client, Block, NimbusCIDP>,
}
impl<Client, Block, AuraCIDP, NimbusCIDP> AgnosticBlockVerifier<Client, Block, AuraCIDP, NimbusCIDP>
where
	Block: BlockT,
{
	pub fn new(
		client: Arc<Client>,
		aura_create_inherent_data_providers: AuraCIDP,
		nimbus_create_inherent_data_providers: NimbusCIDP,
		telemetry: Option<TelemetryHandle>,
	) -> Self
	where
		Client: ProvideRuntimeApi<Block> + Send + Sync + 'static,
		<Client as ProvideRuntimeApi<Block>>::Api: BlockBuilderApi<Block>,
		AuraCIDP: CreateInherentDataProviders<Block, ()> + 'static,
		NimbusCIDP: CreateInherentDataProviders<Block, ()> + 'static,
	{
		Self {
			aura_verifier: cumulus_client_consensus_aura::build_verifier(BuildVerifierParams {
				client: client.clone(),
				create_inherent_data_providers: aura_create_inherent_data_providers,
				telemetry,
			}),
			nimbus_verifier: nimbus_consensus::build_verifier(
				client,
				nimbus_create_inherent_data_providers,
			),
		}
	}
}

#[async_trait::async_trait]
impl<Client, Block, AuraCIDP, NimbusCIDP> VerifierT<Block>
	for AgnosticBlockVerifier<Client, Block, AuraCIDP, NimbusCIDP>
where
	Block: BlockT,
	Client: ProvideRuntimeApi<Block> + Send + Sync,
	<Client as ProvideRuntimeApi<Block>>::Api: BlockBuilderApi<Block> + AuraApi<Block, AuraId>,
	AuraCIDP: CreateInherentDataProviders<Block, ()> + 'static,
	<AuraCIDP as CreateInherentDataProviders<Block, ()>>::InherentDataProviders:
		InherentDataProviderExt,
	NimbusCIDP: CreateInherentDataProviders<Block, ()>,
	Client: sc_client_api::AuxStore + sc_client_api::BlockOf,
{
	/// Verify the given block data and return the BlockImportParams to continue
	/// the block import process.
	async fn verify(
		&mut self,
		block_params: BlockImportParams<Block>,
	) -> Result<BlockImportParams<Block>, String> {
		match get_consensus_kind::<Block>(&block_params.header) {
			ConsensusKind::Aura => self.aura_verifier.verify(block_params).await,
			ConsensusKind::Nimbus => self.nimbus_verifier.verify(block_params).await,
		}
	}
}

/// Start an import queue for a Cumulus collator that does not uses any special authoring logic.
pub fn import_queue<Client, Backend, Block: BlockT, InnerBI>(
	client: Arc<Client>,
	backend: Arc<Backend>,
	block_import: InnerBI,
	spawner: &impl sp_core::traits::SpawnEssentialNamed,
	registry: Option<&substrate_prometheus_endpoint::Registry>,
	telemetry: Option<TelemetryHandle>,
) -> ClientResult<BasicQueue<Block>>
where
	InnerBI: BlockImport<Block, Error = ConsensusError> + Send + Sync + 'static,
	Client::Api: BlockBuilderApi<Block>,
	Client: ProvideRuntimeApi<Block> + Send + Sync + 'static,
	Client: sc_client_api::AuxStore + sc_client_api::UsageProvider<Block>,
	Client: HeaderBackend<Block> + sc_client_api::BlockOf,
	<Client as ProvideRuntimeApi<Block>>::Api: BlockBuilderApi<Block> + AuraApi<Block, AuraId>,
	Backend: sc_client_api::Backend<Block> + 'static,
{
	let verifier = AgnosticBlockVerifier::new(
		client.clone(),
		move |_, _| {
			let client2 = client.clone();
			async move {
				let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
				let slot_duration =
					cumulus_client_consensus_aura::slot_duration(&*client2).unwrap();
				let slot = sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
					*timestamp,
					slot_duration,
				);
				Ok((slot, timestamp))
			}
		},
		move |_, _| async move {
			let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
			Ok((timestamp,))
		},
		telemetry,
	);
	Ok(BasicQueue::new(
		verifier,
		Box::new(cumulus_client_consensus_common::ParachainBlockImport::new(block_import, backend)),
		None,
		spawner,
		registry,
	))
}
