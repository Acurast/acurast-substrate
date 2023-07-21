//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.

// std
use std::{sync::Arc, time::Duration};

use cumulus_client_cli::CollatorOptions;
// Cumulus Imports
use cumulus_client_consensus_common::ParachainConsensus;
use cumulus_client_service::{
	build_network, build_relay_chain_interface, prepare_node_config, start_collator,
	start_full_node, BuildNetworkParams, StartCollatorParams, StartFullNodeParams,
};
use cumulus_primitives_core::ParaId;
use cumulus_relay_chain_interface::RelayChainInterface;
// Substrate Imports
use frame_benchmarking_cli::SUBSTRATE_REFERENCE_HARDWARE;
use pallet_acurast_hyperdrive_outgoing::{
	instances::tezos::TargetChainTezos, mmr_gadget::MmrGadget,
};
use sc_chain_spec::ChainSpec;
use sc_consensus::ImportQueue;
use sc_executor::NativeElseWasmExecutor;
use sc_network::NetworkBlock;
use sc_network_sync::SyncingService;
use sc_service::{Configuration, PartialComponents, TFullBackend, TFullClient, TaskManager};
use sc_telemetry::{Telemetry, TelemetryHandle, TelemetryWorker, TelemetryWorkerHandle};
use sp_api::ConstructRuntimeApi;
use sp_keystore::SyncCryptoStorePtr;
use substrate_prometheus_endpoint::Registry;

#[cfg(feature = "proof-of-authority")]
use cumulus_client_consensus_aura::{AuraConsensus, BuildAuraConsensusParams, SlotProportion};
#[cfg(feature = "proof-of-authority")]
use cumulus_client_consensus_common::ParachainBlockImport as TParachainBlockImport;

// Local Runtime Types
#[cfg(feature = "acurast-kusama")]
pub use acurast_kusama_runtime;
#[cfg(feature = "acurast-local")]
pub use acurast_rococo_runtime as acurast_local_runtime;
#[cfg(feature = "acurast-dev")]
pub use acurast_rococo_runtime as acurast_dev_runtime;
#[cfg(feature = "acurast-rococo")]
pub use acurast_rococo_runtime;
use acurast_runtime_common::opaque::Block;

// Aura
#[cfg(feature = "proof-of-stake")]
pub use sp_consensus_aura::sr25519::AuthorityId as AuraId;

// Nimbus
#[cfg(feature = "proof-of-stake")]
use nimbus_consensus::{BuildNimbusConsensusParams, NimbusConsensus};

use crate::client::{ClientVariant, RuntimeApiCollection};

/// The exhaustive enum of Acurast networks.
#[derive(Clone)]
pub enum NetworkVariant {
	#[cfg(feature = "acurast-local")]
	Local,
	#[cfg(feature = "acurast-dev")]
	Dev,
	#[cfg(feature = "acurast-rococo")]
	Rococo,
	#[cfg(feature = "acurast-kusama")]
	Kusama,
}

impl From<ClientVariant> for NetworkVariant {
	fn from(value: ClientVariant) -> Self {
		match value {
			#[cfg(feature = "acurast-local")]
			ClientVariant::Local(_) => NetworkVariant::Local,
			#[cfg(feature = "acurast-dev")]
			ClientVariant::Dev(_) => NetworkVariant::Dev,
			#[cfg(feature = "acurast-rococo")]
			ClientVariant::Rococo(_) => NetworkVariant::Rococo,
			#[cfg(feature = "acurast-kusama")]
			ClientVariant::Kusama(_) => NetworkVariant::Kusama,
		}
	}
}

/// Can be called for a `Configuration` to check if it is a configuration for
/// one of the Acurast networks.
pub trait IdentifyVariant {
	/// Returns the [`NetworkVariant`] of an Acurast network for a configuration.
	fn variant(&self) -> NetworkVariant;
}

impl IdentifyVariant for Box<dyn ChainSpec> {
	fn variant(&self) -> NetworkVariant {
		match self.id() {
			#[cfg(feature = "acurast-local")]
			id if id.contains("local") => NetworkVariant::Local,
			#[cfg(feature = "acurast-dev")]
			id if id.contains("dev") => NetworkVariant::Dev,
			#[cfg(feature = "acurast-rococo")]
			id if id.contains("rococo") => NetworkVariant::Rococo,
			#[cfg(feature = "acurast-kusama")]
			id if id.contains("kusama") => NetworkVariant::Kusama,
			_ => panic!("invalid chain spec"),
		}
	}
}

/// Native executor type for Acurast Local.
#[cfg(feature = "acurast-local")]
pub struct AcurastLocalNativeExecutor;

#[cfg(feature = "acurast-local")]
impl sc_executor::NativeExecutionDispatch for AcurastLocalNativeExecutor {
	type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;

	fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
		acurast_rococo_runtime::api::dispatch(method, data)
	}

	fn native_version() -> sc_executor::NativeVersion {
		acurast_rococo_runtime::native_version()
	}
}

/// Native executor type for Acurast Development.
#[cfg(feature = "acurast-dev")]
pub struct AcurastDevNativeExecutor;

#[cfg(feature = "acurast-dev")]
impl sc_executor::NativeExecutionDispatch for AcurastDevNativeExecutor {
	type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;

	fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
		acurast_rococo_runtime::api::dispatch(method, data)
	}

	fn native_version() -> sc_executor::NativeVersion {
		acurast_rococo_runtime::native_version()
	}
}

/// Native executor type for Acurast Rococo.
#[cfg(feature = "acurast-rococo")]
pub struct AcurastRococoNativeExecutor;

#[cfg(feature = "acurast-rococo")]
impl sc_executor::NativeExecutionDispatch for AcurastRococoNativeExecutor {
	type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;

	fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
		acurast_rococo_runtime::api::dispatch(method, data)
	}

	fn native_version() -> sc_executor::NativeVersion {
		acurast_rococo_runtime::native_version()
	}
}

/// Native executor type for Acurast Kusama.
#[cfg(feature = "acurast-kusama")]
pub struct AcurastKusamaNativeExecutor;

#[cfg(feature = "acurast-kusama")]
impl sc_executor::NativeExecutionDispatch for AcurastKusamaNativeExecutor {
	type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;

	fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
		acurast_rococo_runtime::api::dispatch(method, data)
	}

	fn native_version() -> sc_executor::NativeVersion {
		acurast_rococo_runtime::native_version()
	}
}

pub type ParachainClient<RuntimeApi, Executor> =
	TFullClient<Block, RuntimeApi, NativeElseWasmExecutor<Executor>>;

pub type ParachainBackend = TFullBackend<Block>;

#[cfg(feature = "proof-of-authority")]
type ParachainBlockImport<RuntimeApi, Executor> =
	TParachainBlockImport<Block, Arc<ParachainClient<RuntimeApi, Executor>>, ParachainBackend>;

/// Starts a `ServiceBuilder` for a full service.
///
/// Use this macro if you don't actually need the full service, but just the builder in order to
/// be able to perform chain operations.
#[cfg(feature = "proof-of-authority")]
pub fn new_partial<RuntimeApi, Executor>(
	config: &Configuration,
) -> Result<
	PartialComponents<
		ParachainClient<RuntimeApi, Executor>,
		ParachainBackend,
		(),
		sc_consensus::DefaultImportQueue<Block, ParachainClient<RuntimeApi, Executor>>,
		sc_transaction_pool::FullPool<Block, ParachainClient<RuntimeApi, Executor>>,
		(
			ParachainBlockImport<RuntimeApi, Executor>,
			Option<Telemetry>,
			Option<TelemetryWorkerHandle>,
		),
	>,
	sc_service::Error,
>
where
	RuntimeApi:
		ConstructRuntimeApi<Block, ParachainClient<RuntimeApi, Executor>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: RuntimeApiCollection<
		StateBackend = sc_client_api::StateBackendFor<ParachainBackend, Block>,
	>,
	Executor: sc_executor::NativeExecutionDispatch + 'static,
{
	let telemetry = config
		.telemetry_endpoints
		.clone()
		.filter(|x| !x.is_empty())
		.map(|endpoints| -> Result<_, sc_telemetry::Error> {
			let worker = TelemetryWorker::new(16)?;
			let telemetry = worker.handle().new_telemetry(endpoints);
			Ok((worker, telemetry))
		})
		.transpose()?;

	let executor = NativeElseWasmExecutor::<Executor>::new(
		config.wasm_method,
		config.default_heap_pages,
		config.max_runtime_instances,
		config.runtime_cache_size,
	);

	let (client, backend, keystore_container, task_manager) =
		sc_service::new_full_parts::<Block, RuntimeApi, _>(
			config,
			telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
			executor,
		)?;
	let client = Arc::new(client);

	let telemetry_worker_handle = telemetry.as_ref().map(|(worker, _)| worker.handle());

	let telemetry = telemetry.map(|(worker, telemetry)| {
		task_manager.spawn_handle().spawn("telemetry", None, worker.run());
		telemetry
	});

	let transaction_pool = sc_transaction_pool::BasicPool::new_full(
		config.transaction_pool.clone(),
		config.role.is_authority().into(),
		config.prometheus_registry(),
		task_manager.spawn_essential_handle(),
		client.clone(),
	);

	let block_import = ParachainBlockImport::new(client.clone(), backend.clone());

	let import_queue = build_import_queue::<RuntimeApi, Executor>(
		client.clone(),
		block_import.clone(),
		config,
		telemetry.as_ref().map(|telemetry| telemetry.handle()),
		&task_manager,
	)?;

	let is_offchain_indexing_enabled = config.offchain_worker.indexing_enabled;

	if is_offchain_indexing_enabled {
		task_manager.spawn_handle().spawn_blocking(
			"mmr-gadget",
			None,
			MmrGadget::start(
				client.clone(),
				backend.clone(),
				pallet_acurast_hyperdrive_outgoing::instances::tezos::INDEXING_PREFIX.to_vec(),
				pallet_acurast_hyperdrive_outgoing::instances::tezos::TEMP_INDEXING_PREFIX.to_vec(),
			),
		);
	}

	Ok(PartialComponents {
		backend,
		client,
		import_queue,
		keystore_container,
		task_manager,
		transaction_pool,
		select_chain: (),
		other: (block_import, telemetry, telemetry_worker_handle),
	})
}

/// Starts a `ServiceBuilder` for a full service.
///
/// Use this macro if you don't actually need the full service, but just the builder in order to
/// be able to perform chain operations.
#[cfg(feature = "proof-of-stake")]
pub fn new_partial<RuntimeApi, Executor>(
	config: &Configuration,
) -> Result<
	PartialComponents<
		ParachainClient<RuntimeApi, Executor>,
		ParachainBackend,
		(),
		sc_consensus::DefaultImportQueue<Block, ParachainClient<RuntimeApi, Executor>>,
		sc_transaction_pool::FullPool<Block, ParachainClient<RuntimeApi, Executor>>,
		(Option<Telemetry>, Option<TelemetryWorkerHandle>),
	>,
	sc_service::Error,
>
where
	RuntimeApi:
		ConstructRuntimeApi<Block, ParachainClient<RuntimeApi, Executor>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: RuntimeApiCollection<StateBackend = sc_client_api::StateBackendFor<ParachainBackend, Block>>
		+ sp_consensus_aura::AuraApi<Block, AuraId>,
	Executor: sc_executor::NativeExecutionDispatch + 'static,
{
	let telemetry = config
		.telemetry_endpoints
		.clone()
		.filter(|x| !x.is_empty())
		.map(|endpoints| -> Result<_, sc_telemetry::Error> {
			let worker = TelemetryWorker::new(16)?;
			let telemetry = worker.handle().new_telemetry(endpoints);
			Ok((worker, telemetry))
		})
		.transpose()?;

	let executor = NativeElseWasmExecutor::<Executor>::new(
		config.wasm_method,
		config.default_heap_pages,
		config.max_runtime_instances,
		config.runtime_cache_size,
	);

	let (client, backend, keystore_container, task_manager) =
		sc_service::new_full_parts::<Block, RuntimeApi, _>(
			config,
			telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
			executor,
		)?;
	let client = Arc::new(client);

	let telemetry_worker_handle = telemetry.as_ref().map(|(worker, _)| worker.handle());

	let telemetry = telemetry.map(|(worker, telemetry)| {
		task_manager.spawn_handle().spawn("telemetry", None, worker.run());
		telemetry
	});

	let transaction_pool = sc_transaction_pool::BasicPool::new_full(
		config.transaction_pool.clone(),
		config.role.is_authority().into(),
		config.prometheus_registry(),
		task_manager.spawn_essential_handle(),
		client.clone(),
	);

	let import_queue = crate::block_verifier::import_queue(
		client.clone(),
		backend.clone(),
		client.clone(),
		&task_manager.spawn_essential_handle(),
		config.prometheus_registry(),
		telemetry.as_ref().map(|telemetry| telemetry.handle()),
	)?;

	let is_offchain_indexing_enabled = config.offchain_worker.indexing_enabled;

	if is_offchain_indexing_enabled {
		task_manager.spawn_handle().spawn_blocking(
			"mmr-gadget",
			None,
			MmrGadget::start(
				client.clone(),
				backend.clone(),
				pallet_acurast_hyperdrive_outgoing::instances::tezos::INDEXING_PREFIX.to_vec(),
				pallet_acurast_hyperdrive_outgoing::instances::tezos::TEMP_INDEXING_PREFIX.to_vec(),
			),
		);
	}

	Ok(PartialComponents {
		backend,
		client,
		import_queue,
		keystore_container,
		task_manager,
		transaction_pool,
		select_chain: (),
		other: (telemetry, telemetry_worker_handle),
	})
}

/// Start a node with the given parachain `Configuration` and relay chain `Configuration`.
///
/// This is the actual implementation that is abstract over the executor and the runtime api.
#[cfg(feature = "proof-of-authority")]
#[sc_tracing::logging::prefix_logs_with("Parachain")]
async fn start_node_impl<RuntimeApi, Executor, BIC>(
	parachain_config: Configuration,
	polkadot_config: Configuration,
	collator_options: CollatorOptions,
	para_id: ParaId,
	hwbench: Option<sc_sysinfo::HwBench>,
	build_consensus: BIC,
) -> sc_service::error::Result<(TaskManager, Arc<ParachainClient<RuntimeApi, Executor>>)>
where
	RuntimeApi:
		ConstructRuntimeApi<Block, ParachainClient<RuntimeApi, Executor>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: RuntimeApiCollection<
		StateBackend = sc_client_api::StateBackendFor<ParachainBackend, Block>,
	>,
	Executor: sc_executor::NativeExecutionDispatch + 'static,
	BIC: FnOnce(
		//  client
		Arc<TFullClient<Block, RuntimeApi, NativeElseWasmExecutor<Executor>>>,
		// 	block_import
		Arc<ParachainBlockImport<RuntimeApi, Executor>>,
		// 	prometheus_registry
		Option<&Registry>,
		// 	telemetry
		Option<TelemetryHandle>,
		// 	task_manager
		&TaskManager,
		// 	relay_chain_interface
		Arc<dyn RelayChainInterface>,
		// 	transaction_pool
		Arc<
			sc_transaction_pool::FullPool<
				Block,
				TFullClient<Block, RuntimeApi, NativeElseWasmExecutor<Executor>>,
			>,
		>,
		// 	sync_oracle
		Arc<SyncingService<Block>>,
		// 	keystore
		SyncCryptoStorePtr,
		// 	force_authoring
		bool,
	) -> Result<Box<dyn ParachainConsensus<Block>>, sc_service::Error>,
{
	let parachain_config = prepare_node_config(parachain_config);

	let params = new_partial(&parachain_config)?;
	let (block_import, mut telemetry, telemetry_worker_handle) = params.other;

	let client = params.client.clone();
	let backend = params.backend.clone();
	let mut task_manager = params.task_manager;

	let (relay_chain_interface, collator_key) = build_relay_chain_interface(
		polkadot_config,
		&parachain_config,
		telemetry_worker_handle,
		&mut task_manager,
		collator_options.clone(),
		hwbench.clone(),
	)
	.await
	.map_err(|e| sc_service::Error::Application(Box::new(e) as Box<_>))?;

	let force_authoring = parachain_config.force_authoring;
	let validator = parachain_config.role.is_authority();
	let prometheus_registry = parachain_config.prometheus_registry().cloned();
	let transaction_pool = params.transaction_pool.clone();
	let import_queue_service = params.import_queue.service();

	let (network, system_rpc_tx, tx_handler_controller, start_network, sync_service) =
		build_network(BuildNetworkParams {
			parachain_config: &parachain_config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			para_id,
			spawn_handle: task_manager.spawn_handle(),
			relay_chain_interface: relay_chain_interface.clone(),
			import_queue: params.import_queue,
		})
		.await?;

	if parachain_config.offchain_worker.enabled {
		sc_service::build_offchain_workers(
			&parachain_config,
			task_manager.spawn_handle(),
			client.clone(),
			network.clone(),
		);
	}

	let rpc_builder = {
		let client = client.clone();
		let transaction_pool = transaction_pool.clone();

		Box::new(move |deny_unsafe, _| {
			let deps = crate::rpc::FullDeps {
				client: client.clone(),
				pool: transaction_pool.clone(),
				deny_unsafe,
			};

			crate::rpc::create_full::<TargetChainTezos, _, _>(deps).map_err(Into::into)
		})
	};

	sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		rpc_builder,
		client: client.clone(),
		transaction_pool: transaction_pool.clone(),
		task_manager: &mut task_manager,
		config: parachain_config,
		keystore: params.keystore_container.sync_keystore(),
		backend: backend.clone(),
		network: network.clone(),
		sync_service: sync_service.clone(),
		system_rpc_tx,
		tx_handler_controller,
		telemetry: telemetry.as_mut(),
	})?;

	if let Some(hwbench) = hwbench {
		sc_sysinfo::print_hwbench(&hwbench);
		// Here you can check whether the hardware meets your chains' requirements. Putting a link
		// in there and swapping out the requirements for your own are probably a good idea. The
		// requirements for a para-chain are dictated by its relay-chain.
		if !SUBSTRATE_REFERENCE_HARDWARE.check_hardware(&hwbench) && validator {
			log::warn!(
				"⚠️  The hardware does not meet the minimal requirements for role 'Authority'."
			);
		}

		if let Some(ref mut telemetry) = telemetry {
			let telemetry_handle = telemetry.handle();
			task_manager.spawn_handle().spawn(
				"telemetry_hwbench",
				None,
				sc_sysinfo::initialize_hwbench_telemetry(telemetry_handle, hwbench),
			);
		}
	}

	let announce_block = {
		let sync_service = sync_service.clone();
		Arc::new(move |hash, data| sync_service.announce_block(hash, data))
	};

	let relay_chain_slot_duration = Duration::from_secs(6);

	let overseer_handle = relay_chain_interface
		.overseer_handle()
		.map_err(|e| sc_service::Error::Application(Box::new(e)))?;

	if validator {
		let parachain_consensus = build_consensus(
			client.clone(),
			Arc::new(block_import),
			prometheus_registry.as_ref(),
			telemetry.as_ref().map(|t| t.handle()),
			&task_manager,
			relay_chain_interface.clone(),
			transaction_pool,
			sync_service,
			params.keystore_container.sync_keystore(),
			force_authoring,
		)?;

		let spawner = task_manager.spawn_handle();
		let params = StartCollatorParams {
			para_id,
			block_status: client.clone(),
			announce_block,
			client: client.clone(),
			task_manager: &mut task_manager,
			relay_chain_interface,
			spawner,
			parachain_consensus,
			import_queue: import_queue_service,
			collator_key: collator_key.expect("Command line arguments do not allow this. qed"),
			relay_chain_slot_duration,
			recovery_handle: Box::new(overseer_handle),
		};

		start_collator(params).await?;
	} else {
		let params = StartFullNodeParams {
			client: client.clone(),
			announce_block,
			task_manager: &mut task_manager,
			para_id,
			relay_chain_interface,
			relay_chain_slot_duration,
			import_queue: import_queue_service,
			recovery_handle: Box::new(overseer_handle),
		};

		start_full_node(params)?;
	}

	start_network.start_network();

	Ok((task_manager, client))
}

/// Start a node with the given parachain `Configuration` and relay chain `Configuration`.
///
/// This is the actual implementation that is abstract over the executor and the runtime api.
#[cfg(feature = "proof-of-stake")]
#[sc_tracing::logging::prefix_logs_with("Parachain")]
async fn start_node_impl<RuntimeApi, Executor, BIC>(
	parachain_config: Configuration,
	polkadot_config: Configuration,
	collator_options: CollatorOptions,
	para_id: ParaId,
	hwbench: Option<sc_sysinfo::HwBench>,
	build_consensus: BIC,
) -> sc_service::error::Result<(TaskManager, Arc<ParachainClient<RuntimeApi, Executor>>)>
where
	RuntimeApi:
		ConstructRuntimeApi<Block, ParachainClient<RuntimeApi, Executor>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: RuntimeApiCollection<StateBackend = sc_client_api::StateBackendFor<ParachainBackend, Block>>
		+ sp_consensus_aura::AuraApi<Block, AuraId>,
	Executor: sc_executor::NativeExecutionDispatch + 'static,
	BIC: FnOnce(
		//  client
		Arc<TFullClient<Block, RuntimeApi, NativeElseWasmExecutor<Executor>>>,
		// 	backend
		Arc<sc_client_db::Backend<Block>>,
		// 	prometheus_registry
		Option<&Registry>,
		// 	telemetry
		Option<TelemetryHandle>,
		// 	task_manager
		&TaskManager,
		// 	relay_chain_interface
		Arc<dyn RelayChainInterface>,
		// 	transaction_pool
		Arc<
			sc_transaction_pool::FullPool<
				Block,
				TFullClient<Block, RuntimeApi, NativeElseWasmExecutor<Executor>>,
			>,
		>,
		// 	sync_oracle
		Arc<SyncingService<Block>>,
		// 	keystore
		SyncCryptoStorePtr,
		// 	force_authoring
		bool,
	) -> Result<Box<dyn ParachainConsensus<Block>>, sc_service::Error>,
{
	let parachain_config = prepare_node_config(parachain_config);

	let params = new_partial::<RuntimeApi, Executor>(&parachain_config)?;
	let (mut telemetry, telemetry_worker_handle) = params.other;

	let client = params.client.clone();
	let backend = params.backend.clone();
	let mut task_manager = params.task_manager;

	let (relay_chain_interface, collator_key) = build_relay_chain_interface(
		polkadot_config,
		&parachain_config,
		telemetry_worker_handle,
		&mut task_manager,
		collator_options.clone(),
		hwbench.clone(),
	)
	.await
	.map_err(|e| sc_service::Error::Application(Box::new(e) as Box<_>))?;

	let force_authoring = parachain_config.force_authoring;
	let validator = parachain_config.role.is_authority();
	let prometheus_registry = parachain_config.prometheus_registry().cloned();
	let transaction_pool = params.transaction_pool.clone();
	let import_queue_service = params.import_queue.service();

	let (network, system_rpc_tx, tx_handler_controller, start_network, sync_service) =
		build_network(BuildNetworkParams {
			parachain_config: &parachain_config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			para_id,
			spawn_handle: task_manager.spawn_handle(),
			relay_chain_interface: relay_chain_interface.clone(),
			import_queue: params.import_queue,
		})
		.await?;

	if parachain_config.offchain_worker.enabled {
		sc_service::build_offchain_workers(
			&parachain_config,
			task_manager.spawn_handle(),
			client.clone(),
			network.clone(),
		);
	}

	let rpc_builder = {
		let client = client.clone();
		let transaction_pool = transaction_pool.clone();

		Box::new(move |deny_unsafe, _| {
			let deps = crate::rpc::FullDeps {
				client: client.clone(),
				pool: transaction_pool.clone(),
				deny_unsafe,
			};

			crate::rpc::create_full::<TargetChainTezos, _, _>(deps).map_err(Into::into)
		})
	};

	sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		rpc_builder,
		client: client.clone(),
		transaction_pool: transaction_pool.clone(),
		task_manager: &mut task_manager,
		config: parachain_config,
		keystore: params.keystore_container.sync_keystore(),
		backend: backend.clone(),
		network: network.clone(),
		sync_service: sync_service.clone(),
		system_rpc_tx,
		tx_handler_controller,
		telemetry: telemetry.as_mut(),
	})?;

	if let Some(hwbench) = hwbench {
		sc_sysinfo::print_hwbench(&hwbench);
		// Here you can check whether the hardware meets your chains' requirements. Putting a link
		// in there and swapping out the requirements for your own are probably a good idea. The
		// requirements for a para-chain are dictated by its relay-chain.
		if !SUBSTRATE_REFERENCE_HARDWARE.check_hardware(&hwbench) && validator {
			log::warn!(
				"⚠️  The hardware does not meet the minimal requirements for role 'Authority'."
			);
		}

		if let Some(ref mut telemetry) = telemetry {
			let telemetry_handle = telemetry.handle();
			task_manager.spawn_handle().spawn(
				"telemetry_hwbench",
				None,
				sc_sysinfo::initialize_hwbench_telemetry(telemetry_handle, hwbench),
			);
		}
	}

	let announce_block = {
		let sync_service = sync_service.clone();
		Arc::new(move |hash, data| sync_service.announce_block(hash, data))
	};

	let relay_chain_slot_duration = Duration::from_secs(6);

	let overseer_handle = relay_chain_interface
		.overseer_handle()
		.map_err(|e| sc_service::Error::Application(Box::new(e)))?;

	if validator {
		let parachain_consensus = build_consensus(
			client.clone(),
			backend,
			prometheus_registry.as_ref(),
			telemetry.as_ref().map(|t| t.handle()),
			&task_manager,
			relay_chain_interface.clone(),
			transaction_pool,
			sync_service,
			params.keystore_container.sync_keystore(),
			force_authoring,
		)?;

		let spawner = task_manager.spawn_handle();
		let params = StartCollatorParams {
			para_id,
			block_status: client.clone(),
			announce_block,
			client: client.clone(),
			task_manager: &mut task_manager,
			relay_chain_interface,
			spawner,
			parachain_consensus,
			import_queue: import_queue_service,
			collator_key: collator_key.expect("Command line arguments do not allow this. qed"),
			relay_chain_slot_duration,
			recovery_handle: Box::new(overseer_handle),
		};

		start_collator(params).await?;
	} else {
		let params = StartFullNodeParams {
			client: client.clone(),
			announce_block,
			task_manager: &mut task_manager,
			para_id,
			relay_chain_interface,
			relay_chain_slot_duration,
			import_queue: import_queue_service,
			recovery_handle: Box::new(overseer_handle),
		};

		start_full_node(params)?;
	}

	start_network.start_network();

	Ok((task_manager, client))
}

/// Build the import queue for the parachain runtime.
#[cfg(feature = "proof-of-authority")]
fn build_import_queue<RuntimeApi, Executor>(
	client: Arc<ParachainClient<RuntimeApi, Executor>>,
	block_import: ParachainBlockImport<RuntimeApi, Executor>,
	config: &Configuration,
	telemetry: Option<TelemetryHandle>,
	task_manager: &TaskManager,
) -> Result<
	sc_consensus::DefaultImportQueue<Block, ParachainClient<RuntimeApi, Executor>>,
	sc_service::Error,
>
where
	RuntimeApi:
		ConstructRuntimeApi<Block, ParachainClient<RuntimeApi, Executor>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: RuntimeApiCollection<
		StateBackend = sc_client_api::StateBackendFor<ParachainBackend, Block>,
	>,
	Executor: sc_executor::NativeExecutionDispatch + 'static,
{
	let slot_duration = cumulus_client_consensus_aura::slot_duration(&*client)?;

	cumulus_client_consensus_aura::import_queue::<
		sp_consensus_aura::sr25519::AuthorityPair,
		_,
		_,
		_,
		_,
		_,
	>(cumulus_client_consensus_aura::ImportQueueParams {
		block_import,
		client,
		create_inherent_data_providers: move |_, _| async move {
			let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

			let slot =
				sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
					*timestamp,
					slot_duration,
				);

			Ok((slot, timestamp))
		},
		registry: config.prometheus_registry(),
		spawner: &task_manager.spawn_essential_handle(),
		telemetry,
	})
	.map_err(Into::into)
}

/// Start a parachain node.
// Rustfmt wants to format the closure with space identation.
#[rustfmt::skip]
#[cfg(feature = "proof-of-authority")]
pub async fn start_parachain_node<RuntimeApi, Executor>(
	parachain_config: Configuration,
	polkadot_config: Configuration,
	collator_options: CollatorOptions,
	para_id: ParaId,
	// rpc_config: RpcConfig,
	hwbench: Option<sc_sysinfo::HwBench>,
) -> sc_service::error::Result<(TaskManager, Arc<ParachainClient<RuntimeApi, Executor>>)>
	where
		RuntimeApi:
		ConstructRuntimeApi<Block, ParachainClient<RuntimeApi, Executor>> + Send + Sync + 'static,
		RuntimeApi::RuntimeApi: RuntimeApiCollection<
			StateBackend = sc_client_api::StateBackendFor<ParachainBackend, Block>>,
		Executor: sc_executor::NativeExecutionDispatch + 'static,
{
	start_node_impl::<RuntimeApi, Executor, _>(
		parachain_config,
		polkadot_config,
		collator_options,
		para_id,
		// rpc_config
		hwbench,
		|
			client,
			block_import,
			prometheus_registry,
			telemetry,
			task_manager,
			relay_chain_interface,
			transaction_pool,
			sync_oracle,
			keystore,
			force_authoring
		| {
			let slot_duration = cumulus_client_consensus_aura::slot_duration(&*client)?;

			let proposer_factory = sc_basic_authorship::ProposerFactory::with_proof_recording(
				task_manager.spawn_handle(),
				client.clone(),
				transaction_pool,
				prometheus_registry,
				telemetry.clone(),
			);
			let params = BuildAuraConsensusParams {
				proposer_factory,
				create_inherent_data_providers: move |_, (relay_parent, validation_data)| {
					let relay_chain_interface = relay_chain_interface.clone();
					async move {
						let parachain_inherent =
							cumulus_primitives_parachain_inherent::ParachainInherentData::create_at(
								relay_parent,
								&relay_chain_interface,
								&validation_data,
								para_id,
							)
								.await;

						let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

						let slot =
							sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
								*timestamp,
								slot_duration,
							);


						let parachain_inherent = parachain_inherent.ok_or_else(|| {
							Box::<dyn std::error::Error + Send + Sync>::from(
								"Failed to create parachain inherent",
							)
						})?;

						Ok((slot, timestamp, parachain_inherent))
					}
				},
				block_import: (*block_import).clone(),
				para_client: client,
				backoff_authoring_blocks: Option::<()>::None,
				sync_oracle,
				keystore,
				force_authoring,
				slot_duration,
				// We got around 500ms for proposing
				block_proposal_slot_portion: SlotProportion::new(1f32 / 24f32),
				// And a maximum of 750ms if slots are skipped
				max_block_proposal_slot_portion: Some(SlotProportion::new(1f32 / 16f32)),
				telemetry,
			};

			Ok(AuraConsensus::build::<sp_consensus_aura::sr25519::AuthorityPair, _, _, _, _, _, _>(
				params,
			))
		},
	)
		.await
}

/// Start a parachain node.
// Rustfmt wants to format the closure with space identation.
#[rustfmt::skip]
#[cfg(feature = "proof-of-stake")]
pub async fn start_parachain_node<RuntimeApi, Executor>(
	parachain_config: Configuration,
	polkadot_config: Configuration,
	collator_options: CollatorOptions,
	para_id: ParaId,
	// rpc_config: RpcConfig,
	hwbench: Option<sc_sysinfo::HwBench>,
) -> sc_service::error::Result<(TaskManager, Arc<ParachainClient<RuntimeApi, Executor>>)>
where
	RuntimeApi:
		ConstructRuntimeApi<Block, ParachainClient<RuntimeApi, Executor>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: RuntimeApiCollection<
		StateBackend = sc_client_api::StateBackendFor<ParachainBackend, Block>,
	> + sp_consensus_aura::AuraApi<Block, AuraId>,
	Executor: sc_executor::NativeExecutionDispatch + 'static,
{
	start_node_impl::<RuntimeApi, Executor, _>(
		parachain_config,
		polkadot_config,
		collator_options,
		para_id,
		// rpc_config
		hwbench,
		|
			client,
			backend,
		 	prometheus_registry,
		 	telemetry,
			task_manager,
			relay_chain_interface,
			transaction_pool,
			_sync_oracle,
			keystore,
			force_authoring
		| {
			let mut proposer_factory = sc_basic_authorship::ProposerFactory::with_proof_recording(
				task_manager.spawn_handle(),
				client.clone(),
				transaction_pool,
				prometheus_registry,
				telemetry.clone(),
			);
			proposer_factory.set_soft_deadline(sp_runtime::Percent::from_percent(100));

			let params = BuildNimbusConsensusParams {
				para_id,
				proposer_factory,
				create_inherent_data_providers: move |_, (relay_parent, validation_data, _author_id)| {
					let relay_chain_interface = relay_chain_interface.clone();
					async move {
						let parachain_inherent =
							cumulus_primitives_parachain_inherent::ParachainInherentData::create_at(
								relay_parent,
								&relay_chain_interface,
								&validation_data,
								para_id,
							)
								.await;
						let time = sp_timestamp::InherentDataProvider::from_system_time();

						let parachain_inherent = parachain_inherent.ok_or_else(|| {
							Box::<dyn std::error::Error + Send + Sync>::from(
								"Failed to create parachain inherent",
							)
						})?;
						let nimbus_inherent = nimbus_primitives::InherentDataProvider;

						Ok((time, parachain_inherent, nimbus_inherent))
					}
				},
				block_import: client.clone(),
				backend,
				parachain_client: client,
				keystore,
				skip_prediction: force_authoring,
				additional_digests_provider: ()
			};

			Ok(NimbusConsensus::build(params))
		},
	)
	.await
}
