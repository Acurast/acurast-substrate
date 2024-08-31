use std::net::SocketAddr;

use cumulus_primitives_core::ParaId;
use frame_benchmarking_cli::{BenchmarkCmd, SUBSTRATE_REFERENCE_HARDWARE};
use log::info;
use sc_cli::{
	ChainSpec, CliConfiguration, DefaultConfigurationValues, ImportParams, KeystoreParams,
	NetworkParams, Result, SharedParams, SubstrateCli,
};
use sc_service::config::{BasePath, PrometheusConfig};
use sp_runtime::traits::AccountIdConversion;

use crate::{
	chain_spec,
	cli::{Cli, RelayChainCli, RunCmd, Subcommand},
	service::{self, new_partial, IdentifyVariant, NetworkVariant},
};

fn load_spec(id: &str, run_cmd: &RunCmd) -> std::result::Result<Box<dyn ChainSpec>, String> {
	Ok(match id {
		#[cfg(feature = "acurast-local")]
		"acurast-local" => Box::new(chain_spec::local::acurast_local_config("rococo-local")),
		#[cfg(feature = "acurast-dev")]
		"" | "acurast-dev" => Box::new(chain_spec::dev::acurast_development_config()),
		#[cfg(feature = "acurast-rococo")]
		"acurast-rococo" => Box::new(chain_spec::rococo::acurast_rococo_config()),
		#[cfg(feature = "acurast-kusama")]
		"acurast-kusama" => Box::new(chain_spec::kusama::acurast_kusama_config()),
		#[cfg(feature = "acurast-mainnet")]
		"acurast-mainnet" => Box::new(chain_spec::mainnet::acurast_config()),

		// Specs provided as json use the dev runtime by default but flags can be used to specify which runtime to use
		path => {
			let path = std::path::PathBuf::from(path);

			// first check if any runtime got explicitly forced by command line argument
			#[cfg(feature = "acurast-local")]
			if run_cmd.use_local {
				#[rustfmt::skip]
				return Ok(Box::new(chain_spec::local::ChainSpec::from_json_file(path)?));
			}
			#[cfg(feature = "acurast-dev")]
			if run_cmd.use_dev {
				#[rustfmt::skip]
				return Ok(Box::new(chain_spec::dev::ChainSpec::from_json_file(path)?));
			}
			#[cfg(feature = "acurast-rococo")]
			if run_cmd.use_rococo {
				#[rustfmt::skip]
				return Ok(Box::new(chain_spec::rococo::ChainSpec::from_json_file(path)?));
			}
			#[cfg(feature = "acurast-kusama")]
			if run_cmd.use_kusama {
				#[rustfmt::skip]
				return Ok(Box::new(chain_spec::kusama::ChainSpec::from_json_file(path)?));
			}

			#[cfg(feature = "acurast-mainnet")]
			if run_cmd.use_mainnet {
				#[rustfmt::skip]
				return Ok(Box::new(chain_spec::mainnet::ChainSpec::from_json_file(path)?));
			}

			// fallback to guessing runtime from provided chain_spec's file name
			let starts_with = |element: &str| {
				path.file_name()
					.and_then(|f| f.to_str().map(|s| s.starts_with(&element)))
					.unwrap_or(false)
			};

			if starts_with("acurast-local") {
				#[cfg(feature = "acurast-local")]
				#[rustfmt::skip]
				return Ok(Box::new(chain_spec::local::ChainSpec::from_json_file(path)?));
				#[cfg(not(feature = "acurast-local"))]
				panic!("guessed runtime from file name as 'acurast-local' but feature 'acurast-local' was not included when building the node");
			} else if starts_with("acurast-dev") {
				#[cfg(feature = "acurast-dev")]
				#[rustfmt::skip]
				return Ok(Box::new(chain_spec::dev::ChainSpec::from_json_file(path)?));
				#[cfg(not(feature = "acurast-dev"))]
				panic!("guessed runtime from file name as 'acurast-dev' but feature 'acurast-dev' was not included when building the node");
			} else if starts_with("acurast-rococo") {
				#[cfg(feature = "acurast-rococo")]
				#[rustfmt::skip]
				return Ok(Box::new(chain_spec::rococo::ChainSpec::from_json_file(path)?));
				#[cfg(not(feature = "acurast-rococo"))]
				panic!("guessed runtime from file name as 'acurast-rococo' but feature 'acurast-rococo' was not included when building the node");
			} else if starts_with("acurast-kusama") {
				#[cfg(feature = "acurast-kusama")]
				#[rustfmt::skip]
				return Ok(Box::new(chain_spec::kusama::ChainSpec::from_json_file(path)?));
				#[cfg(not(feature = "acurast-kusama"))]
				panic!("guessed runtime from file name as 'acurast-kusama' but feature 'acurast-kusama' was not included when building the node");
			} else if starts_with("acurast-mainnet") {
				#[cfg(feature = "acurast-mainnet")]
				#[rustfmt::skip]
				return Ok(Box::new(chain_spec::mainnet::ChainSpec::from_json_file(path)?));
				#[cfg(not(feature = "acurast-mainnet"))]
				panic!("guessed runtime from file name as 'acurast-mainnet' but feature 'acurast-mainnet' was not included when building the node");
			} else {
				panic!("could not derive chain spec: non of the --rococo-runtime --kusama-runtime flags was used and the runtime was not clear from the file name");
			}
		},
	})
}

impl SubstrateCli for Cli {
	fn impl_name() -> String {
		"Acurast Parachain Collator".into()
	}

	fn impl_version() -> String {
		env!("SUBSTRATE_CLI_IMPL_VERSION").into()
	}

	fn description() -> String {
		format!(
			"Acurast Collator\n\nThe command-line arguments provided first will be \
		passed to the parachain node, while the arguments provided after -- will be passed \
		to the relay chain node.\n\n\
		{} <parachain-args> -- <relay-chain-args>",
			Self::executable_name()
		)
	}

	fn author() -> String {
		env!("CARGO_PKG_AUTHORS").into()
	}

	fn support_url() -> String {
		"https://github.com/Acurast/acurast-substrate/issues/new".into()
	}

	fn copyright_start_year() -> i32 {
		2020
	}

	fn load_spec(&self, id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
		load_spec(id, &self.run)
	}
}

impl SubstrateCli for RelayChainCli {
	fn impl_name() -> String {
		"Parachain Collator Template".into()
	}

	fn impl_version() -> String {
		env!("SUBSTRATE_CLI_IMPL_VERSION").into()
	}

	fn description() -> String {
		format!(
			"Acurast Collator\n\nThe command-line arguments provided first will be \
		passed to the parachain node, while the arguments provided after -- will be passed \
		to the relay chain node.\n\n\
		{} <parachain-args> -- <relay-chain-args>",
			Self::executable_name()
		)
	}

	fn author() -> String {
		env!("CARGO_PKG_AUTHORS").into()
	}

	fn support_url() -> String {
		"https://github.com/Acurast/acurast-substrate/issues/new".into()
	}

	fn copyright_start_year() -> i32 {
		2020
	}

	fn load_spec(&self, id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
		polkadot_cli::Cli::from_iter([RelayChainCli::executable_name()].iter()).load_spec(id)
	}
}

macro_rules! construct_async_run {
	{< $runtime:ty, $executor:ty > ( |$components:ident, $cli:ident, $cmd:ident, $config:ident| $( $code:tt )* )} => {{
		let runner = $cli.create_runner($cmd)?;
		runner.async_run(|$config| {
			let $components = new_partial::<$runtime, $executor>(&$config)?;
			let task_manager = $components.task_manager;
			{ $( $code )* }.map(|v| (v, task_manager))
		})
	}}
}

/// Parse command line arguments into service configuration.
pub fn run() -> Result<()> {
	let cli = Cli::from_args();

	match &cli.subcommand {
		Some(Subcommand::BuildSpec(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run(config.chain_spec, config.network))
		},
		Some(Subcommand::CheckBlock(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			let chain_spec = &runner.config().chain_spec;
			match chain_spec.variant() {
				#[cfg(any(feature = "acurast-local", feature = "acurast-dev", feature = "acurast-rococo"))]
				NetworkVariant::Testnet => {
					construct_async_run! {
						<acurast_rococo_runtime::RuntimeApi, service::testnet::AcurastExecutor>(|components, cli, cmd, config| {
							Ok(cmd.run(components.client, components.import_queue))
						})
					}
				},
				#[cfg(feature = "acurast-kusama")]
				NetworkVariant::Canary => {
					construct_async_run! {
						<acurast_kusama_runtime::RuntimeApi, service::canary::AcurastExecutor>(|components, cli, cmd, config| {
							Ok(cmd.run(components.client, components.import_queue))
						})
					}
				},
				#[cfg(feature = "acurast-mainnet")]
				NetworkVariant::Mainnet => {
					construct_async_run! {
						<acurast_mainnet_runtime::RuntimeApi, service::mainnet::AcurastExecutor>(|components, cli, cmd, config| {
							Ok(cmd.run(components.client, components.import_queue))
						})
					}
				},
			}
		},
		Some(Subcommand::ExportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			let chain_spec = &runner.config().chain_spec;
			match chain_spec.variant() {
				#[cfg(any(feature = "acurast-local", feature = "acurast-dev", feature = "acurast-rococo"))]
				NetworkVariant::Testnet => {
					construct_async_run! {
						<acurast_rococo_runtime::RuntimeApi, service::testnet::AcurastExecutor>(|components, cli, cmd, config| {
							Ok(cmd.run(components.client, config.database))
						})
					}
				},
				#[cfg(feature = "acurast-kusama")]
				NetworkVariant::Canary => {
					construct_async_run! {
						<acurast_kusama_runtime::RuntimeApi, service::canary::AcurastExecutor>(|components, cli, cmd, config| {
							Ok(cmd.run(components.client, config.database))
						})
					}
				},
				#[cfg(feature = "acurast-mainnet")]
				NetworkVariant::Mainnet => {
					construct_async_run! {
						<acurast_mainnet_runtime::RuntimeApi, service::mainnet::AcurastExecutor>(|components, cli, cmd, config| {
							Ok(cmd.run(components.client, config.database))
						})
					}
				},
			}
		},
		Some(Subcommand::ExportState(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			let chain_spec = &runner.config().chain_spec;
			match chain_spec.variant() {
				#[cfg(any(feature = "acurast-local", feature = "acurast-dev", feature = "acurast-rococo"))]
				NetworkVariant::Testnet => {
					construct_async_run! {
						<acurast_rococo_runtime::RuntimeApi, service::testnet::AcurastExecutor>(|components, cli, cmd, config| {
							Ok(cmd.run(components.client, config.chain_spec))
						})
					}
				},
				#[cfg(feature = "acurast-kusama")]
				NetworkVariant::Canary => {
					construct_async_run! {
						<acurast_kusama_runtime::RuntimeApi, service::canary::AcurastExecutor>(|components, cli, cmd, config| {
							Ok(cmd.run(components.client, config.chain_spec))
						})
					}
				},
				#[cfg(feature = "acurast-mainnet")]
				NetworkVariant::Mainnet => {
					construct_async_run! {
						<acurast_mainnet_runtime::RuntimeApi, service::mainnet::AcurastExecutor>(|components, cli, cmd, config| {
							Ok(cmd.run(components.client, config.chain_spec))
						})
					}
				},
			}
		},
		Some(Subcommand::ImportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			let chain_spec = &runner.config().chain_spec;
			match chain_spec.variant() {
				#[cfg(any(feature = "acurast-local", feature = "acurast-dev", feature = "acurast-rococo"))]
				NetworkVariant::Testnet => {
					construct_async_run! {
						<acurast_rococo_runtime::RuntimeApi, service::testnet::AcurastExecutor>(|components, cli, cmd, config| {
							Ok(cmd.run(components.client, components.import_queue))
						})
					}
				},
				#[cfg(feature = "acurast-kusama")]
				NetworkVariant::Canary => {
					construct_async_run! {
						<acurast_kusama_runtime::RuntimeApi, service::canary::AcurastExecutor>(|components, cli, cmd, config| {
							Ok(cmd.run(components.client, components.import_queue))
						})
					}
				},
				#[cfg(feature = "acurast-mainnet")]
				NetworkVariant::Mainnet => {
					construct_async_run! {
						<acurast_mainnet_runtime::RuntimeApi, service::mainnet::AcurastExecutor>(|components, cli, cmd, config| {
							Ok(cmd.run(components.client, components.import_queue))
						})
					}
				},
			}
		},
		Some(Subcommand::Revert(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			let chain_spec = &runner.config().chain_spec;
			match chain_spec.variant() {
				#[cfg(any(feature = "acurast-local", feature = "acurast-dev", feature = "acurast-rococo"))]
				NetworkVariant::Testnet => {
					construct_async_run! {
						<acurast_rococo_runtime::RuntimeApi, service::testnet::AcurastExecutor>(|components, cli, cmd, config| {
							Ok(cmd.run(components.client, components.backend, None))
						})
					}
				},
				#[cfg(feature = "acurast-kusama")]
				NetworkVariant::Canary => {
					construct_async_run! {
						<acurast_kusama_runtime::RuntimeApi, service::canary::AcurastExecutor>(|components, cli, cmd, config| {
							Ok(cmd.run(components.client, components.backend, None))
						})
					}
				},
				#[cfg(feature = "acurast-mainnet")]
				NetworkVariant::Mainnet => {
					construct_async_run! {
						<acurast_mainnet_runtime::RuntimeApi, service::mainnet::AcurastExecutor>(|components, cli, cmd, config| {
							Ok(cmd.run(components.client, components.backend, None))
						})
					}
				},
			}
		},
		Some(Subcommand::PurgeChain(cmd)) => {
			let runner = cli.create_runner(cmd)?;

			runner.sync_run(|config| {
				let polkadot_cli = RelayChainCli::new(
					&config,
					[RelayChainCli::executable_name()].iter().chain(cli.relay_chain_args.iter()),
				);

				let polkadot_config = SubstrateCli::create_configuration(
					&polkadot_cli,
					&polkadot_cli,
					config.tokio_handle.clone(),
				)
				.map_err(|err| format!("Relay chain argument error: {}", err))?;

				cmd.run(config, polkadot_config)
			})
		},
		Some(Subcommand::ExportGenesisState(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(| config| {
				let chain_spec = &config.chain_spec;
				match chain_spec.variant() {
					#[cfg(any(feature = "acurast-local", feature = "acurast-dev", feature = "acurast-rococo"))]
					NetworkVariant::Testnet => {
						let partials = new_partial::<
							acurast_rococo_runtime::RuntimeApi,
							service::testnet::AcurastExecutor,
						>(&config)?;
						cmd.run(partials.client)
					},
					#[cfg(feature = "acurast-kusama")]
					NetworkVariant::Canary => {
						let partials = new_partial::<
							acurast_kusama_runtime::RuntimeApi,
							service::canary::AcurastExecutor,
						>(&config)?;
						cmd.run(partials.client)
					},
					#[cfg(feature = "acurast-mainnet")]
					NetworkVariant::Mainnet => {
						let partials = new_partial::<
							acurast_mainnet_runtime::RuntimeApi,
							service::mainnet::AcurastExecutor,
						>(&config)?;
						cmd.run(partials.client)
					},
				}
			})
		},
		Some(Subcommand::ExportGenesisWasm(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|_config| {
				let spec = cli.load_spec(&cmd.shared_params.chain.clone().unwrap_or_default())?;
				cmd.run(&*spec)
			})
		},
		Some(Subcommand::Benchmark(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			// Switch on the concrete benchmark sub-command-
			match cmd {
				BenchmarkCmd::Pallet(cmd) =>
					if cfg!(feature = "runtime-benchmarks") {
						return runner.sync_run(|config| {
							cmd.run::<acurast_runtime_common::opaque::Block, ()>(config)
						})
					} else {
						Err("Benchmarking wasn't enabled when building the node. \
					You can enable it with `--features runtime-benchmarks`."
							.into())
					},
				BenchmarkCmd::Block(cmd) =>
					runner.sync_run(|config| match config.chain_spec.variant() {
						#[cfg(any(feature = "acurast-local", feature = "acurast-dev", feature = "acurast-rococo"))]
						NetworkVariant::Testnet => {
							let partials = new_partial::<
								acurast_rococo_runtime::RuntimeApi,
								service::testnet::AcurastExecutor,
							>(&config)?;
							cmd.run(partials.client)
						},
						#[cfg(feature = "acurast-kusama")]
						NetworkVariant::Canary => {
							let partials = new_partial::<
								acurast_kusama_runtime::RuntimeApi,
								service::canary::AcurastExecutor,
							>(&config)?;
							cmd.run(partials.client)
						},
						#[cfg(feature = "acurast-mainnet")]
						NetworkVariant::Mainnet => {
							let partials = new_partial::<
								acurast_mainnet_runtime::RuntimeApi,
								service::mainnet::AcurastExecutor,
							>(&config)?;
							cmd.run(partials.client)
						},
					}),
				#[cfg(not(feature = "runtime-benchmarks"))]
				BenchmarkCmd::Storage(_) =>
					return Err(sc_cli::Error::Input(
						"Compile with --features=runtime-benchmarks \
						to enable storage benchmarks."
							.into(),
					)
					.into()),
				#[cfg(feature = "runtime-benchmarks")]
				BenchmarkCmd::Storage(cmd) => runner.sync_run(|config| {
					match config.chain_spec.variant() {
						#[cfg(any(feature = "acurast-local", feature = "acurast-dev", feature = "acurast-rococo"))]
						NetworkVariant::Testnet => {
							let partials = new_partial::<
								acurast_rococo_runtime::RuntimeApi,
								service::testnet::AcurastExecutor,
							>(&config)?;
							let db = partials.backend.expose_db();
							let storage = partials.backend.expose_storage();
							cmd.run(config, partials.client.clone(), db, storage)
						},
						#[cfg(feature = "acurast-kusama")]
						NetworkVariant::Canary => {
							let partials = new_partial::<
								acurast_kusama_runtime::RuntimeApi,
								service::canary::AcurastExecutor,
							>(&config)?;
							let db = partials.backend.expose_db();
							let storage = partials.backend.expose_storage();
							cmd.run(config, partials.client.clone(), db, storage)
						},
						#[cfg(feature = "acurast-mainnet")]
						NetworkVariant::Mainnet => {
							let partials = new_partial::<
								acurast_mainnet_runtime::RuntimeApi,
								service::mainnet::AcurastExecutor,
							>(&config)?;
							let db = partials.backend.expose_db();
							let storage = partials.backend.expose_storage();
							cmd.run(config, partials.client.clone(), db, storage)
						},
					}
				}),
				BenchmarkCmd::Machine(cmd) =>
					runner.sync_run(|config| cmd.run(&config, SUBSTRATE_REFERENCE_HARDWARE.clone())),
				// NOTE: this allows the Client to leniently implement
				// new benchmark commands without requiring a companion MR.
				#[allow(unreachable_patterns)]
				_ => Err("Benchmarking sub-command unsupported".into()),
			}
		},
		Some(Subcommand::TryRuntime) => Err("The `try-runtime` subcommand has been migrated to a standalone CLI (https://github.com/paritytech/try-runtime-cli). It is no longer being maintained here and will be removed entirely some time after January 2024. Please remove this subcommand from your runtime and use the standalone CLI.".into()),
		None => {
			let runner = cli.create_runner(&cli.run.normalize())?;
			let collator_options = cli.run.collator_options();

			runner.run_node_until_exit(|config| async move {
				let hwbench = if !cli.no_hardware_benchmarks {
					config.database.path().map(|database_path| {
						let _ = std::fs::create_dir_all(&database_path);
						sc_sysinfo::gather_hwbench(Some(database_path))
					})
				} else {
					None
				};

				let polkadot_cli = RelayChainCli::new(
					&config,
					[RelayChainCli::executable_name()].iter().chain(cli.relay_chain_args.iter()),
				);

				// allow command line argument to overwrite para_id from chain_spec
				let id = ParaId::from(match cli.run.parachain_id {
					Some(id) => id.clone(),
					None => chain_spec::Extensions::try_get(&*config.chain_spec)
						.map(|e| e.para_id)
						.ok_or_else(|| "Could not find parachain ID in chain-spec.")?,
				});

				let parachain_account =
					AccountIdConversion::<polkadot_primitives::AccountId>::into_account_truncating(
						&id,
					);

				let tokio_handle = config.tokio_handle.clone();
				let polkadot_config =
					SubstrateCli::create_configuration(&polkadot_cli, &polkadot_cli, tokio_handle)
						.map_err(|err| format!("Relay chain argument error: {}", err))?;

				info!("Parachain id: {:?}", id);
				info!("Parachain Account: {}", parachain_account);
				info!("Is collating: {}", if config.role.is_authority() { "yes" } else { "no" });

				match &config.chain_spec.variant() {
					#[cfg(any(feature = "acurast-local", feature = "acurast-dev", feature = "acurast-rococo"))]
					NetworkVariant::Testnet => service::start_parachain_node::<
						acurast_rococo_runtime::RuntimeApi,
						service::testnet::AcurastExecutor,
					>(config, polkadot_config, collator_options, id, cli.run.block_authoring_duration, hwbench)
						.await
						.map(|r| r.0)
						.map_err(Into::into),
					#[cfg(feature = "acurast-kusama")]
					NetworkVariant::Canary => service::start_parachain_node::<
						acurast_kusama_runtime::RuntimeApi,
						service::canary::AcurastExecutor,
					>(config, polkadot_config, collator_options, id, cli.run.block_authoring_duration, hwbench)
						.await
						.map(|r| r.0)
						.map_err(Into::into),
					#[cfg(feature = "acurast-mainnet")]
					NetworkVariant::Mainnet => service::start_parachain_node::<
						acurast_mainnet_runtime::RuntimeApi,
						service::mainnet::AcurastExecutor,
					>(config, polkadot_config, collator_options, id, cli.run.block_authoring_duration, hwbench)
						.await
						.map(|r| r.0)
						.map_err(Into::into),
				}
			})
		},
	}
}

impl DefaultConfigurationValues for RelayChainCli {
	fn p2p_listen_port() -> u16 {
		30334
	}

	fn rpc_listen_port() -> u16 {
		9945
	}

	fn prometheus_listen_port() -> u16 {
		9616
	}
}

impl CliConfiguration<Self> for RelayChainCli {
	fn shared_params(&self) -> &SharedParams {
		self.base.base.shared_params()
	}

	fn import_params(&self) -> Option<&ImportParams> {
		self.base.base.import_params()
	}

	fn network_params(&self) -> Option<&NetworkParams> {
		self.base.base.network_params()
	}

	fn keystore_params(&self) -> Option<&KeystoreParams> {
		self.base.base.keystore_params()
	}

	fn base_path(&self) -> Result<Option<BasePath>> {
		Ok(self
			.shared_params()
			.base_path()?
			.or_else(|| self.base_path.clone().map(Into::into)))
	}

	fn rpc_addr(&self, default_listen_port: u16) -> Result<Option<SocketAddr>> {
		self.base.base.rpc_addr(default_listen_port)
	}

	fn prometheus_config(
		&self,
		default_listen_port: u16,
		chain_spec: &Box<dyn ChainSpec>,
	) -> Result<Option<PrometheusConfig>> {
		self.base.base.prometheus_config(default_listen_port, chain_spec)
	}

	fn init<F>(
		&self,
		_support_url: &String,
		_impl_version: &String,
		_logger_hook: F,
		_config: &sc_service::Configuration,
	) -> Result<()>
	where
		F: FnOnce(&mut sc_cli::LoggerBuilder, &sc_service::Configuration),
	{
		unreachable!("PolkadotCli is never initialized; qed");
	}

	fn chain_id(&self, is_dev: bool) -> Result<String> {
		let chain_id = self.base.base.chain_id(is_dev)?;

		Ok(if chain_id.is_empty() { self.chain_id.clone().unwrap_or_default() } else { chain_id })
	}

	fn role(&self, is_dev: bool) -> Result<sc_service::Role> {
		self.base.base.role(is_dev)
	}

	fn transaction_pool(&self, is_dev: bool) -> Result<sc_service::config::TransactionPoolOptions> {
		self.base.base.transaction_pool(is_dev)
	}

	fn trie_cache_maximum_size(&self) -> Result<Option<usize>> {
		self.base.base.trie_cache_maximum_size()
	}

	fn rpc_methods(&self) -> Result<sc_service::config::RpcMethods> {
		self.base.base.rpc_methods()
	}

	fn rpc_max_connections(&self) -> Result<u32> {
		self.base.base.rpc_max_connections()
	}

	fn rpc_cors(&self, is_dev: bool) -> Result<Option<Vec<String>>> {
		self.base.base.rpc_cors(is_dev)
	}

	fn default_heap_pages(&self) -> Result<Option<u64>> {
		self.base.base.default_heap_pages()
	}

	fn force_authoring(&self) -> Result<bool> {
		self.base.base.force_authoring()
	}

	fn disable_grandpa(&self) -> Result<bool> {
		self.base.base.disable_grandpa()
	}

	fn max_runtime_instances(&self) -> Result<Option<usize>> {
		self.base.base.max_runtime_instances()
	}

	fn announce_block(&self) -> Result<bool> {
		self.base.base.announce_block()
	}

	fn telemetry_endpoints(
		&self,
		chain_spec: &Box<dyn ChainSpec>,
	) -> Result<Option<sc_telemetry::TelemetryEndpoints>> {
		self.base.base.telemetry_endpoints(chain_spec)
	}

	fn node_name(&self) -> Result<String> {
		self.base.base.node_name()
	}
}
