#![allow(clippy::large_enum_variant)]

use std::{path::PathBuf, time::Duration};

/// Sub-commands supported by the collator.
#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
	/// Build a chain specification.
	BuildSpec(sc_cli::BuildSpecCmd),

	/// Validate blocks.
	CheckBlock(sc_cli::CheckBlockCmd),

	/// Export blocks.
	ExportBlocks(sc_cli::ExportBlocksCmd),

	/// Export the state of a given block into a chain spec.
	ExportState(sc_cli::ExportStateCmd),

	/// Import blocks.
	ImportBlocks(sc_cli::ImportBlocksCmd),

	/// Revert the chain to a previous state.
	Revert(sc_cli::RevertCmd),

	/// Remove the whole chain.
	PurgeChain(cumulus_client_cli::PurgeChainCmd),

	/// Export the genesis state of the parachain.
	#[command(alias = "export-genesis-state")]
	ExportGenesisState(cumulus_client_cli::ExportGenesisHeadCommand),

	/// Export the genesis wasm of the parachain.
	ExportGenesisWasm(cumulus_client_cli::ExportGenesisWasmCommand),

	/// Sub-commands concerned with benchmarking.
	/// The pallet benchmarking moved to the `pallet` sub-command.
	#[command(subcommand)]
	Benchmark(frame_benchmarking_cli::BenchmarkCmd),
}

const AFTER_HELP_EXAMPLE: &str = color_print::cstr!(
	r#"<bold><underline>Examples:</></>
   <bold>acurast-node build-spec --disable-default-bootnode > plain-parachain-chainspec.json</>
           Export a chainspec for a local testnet in json format.
   <bold>acurast-node --chain plain-parachain-chainspec.json --tmp -- --chain rococo-local</>
           Launch a full node with chain specification loaded from plain-parachain-chainspec.json.
   <bold>acurast-node</>
           Launch a full node with default parachain <italic>acurast-local</> and relay chain <italic>rococo-local</>.
   <bold>acurast-node --collator</>
           Launch a collator with default parachain <italic>acurast-local</> and relay chain <italic>rococo-local</>.
 "#
);

#[derive(Debug, clap::Parser)]
#[command(
	propagate_version = true,
	args_conflicts_with_subcommands = true,
	subcommand_negates_reqs = true
)]
#[clap(after_help = AFTER_HELP_EXAMPLE)]
pub struct Cli {
	#[command(subcommand)]
	pub subcommand: Option<Subcommand>,

	#[command(flatten)]
	pub run: RunCmd,

	/// Disable automatic hardware benchmarks.
	///
	/// By default these benchmarks are automatically ran at startup and measure
	/// the CPU speed, the memory bandwidth and the disk speed.
	///
	/// The results are then printed out in the logs, and also sent as part of
	/// telemetry, if telemetry is enabled.
	#[arg(long)]
	pub no_hardware_benchmarks: bool,

	/// Relay chain arguments
	#[arg(raw = true)]
	pub relay_chain_args: Vec<String>,
}

#[derive(Debug, clap::Args)]
pub struct TunnelArgs {
	/// Enable the integrated tunnel server.
	#[arg(long)]
	pub tunnel: bool,

	/// Address to bind the tunnel listeners on.
	#[arg(long, default_value = "0.0.0.0")]
	pub tunnel_bind_addr: String,

	/// Port for QUIC + HTTP/2 agent connections.
	#[arg(long, default_value_t = 4433)]
	pub tunnel_api_port: u16,

	/// Port for public (user-facing) connections.
	#[arg(long, default_value_t = 8443)]
	pub tunnel_pub_port: u16,

	/// Port for ACME TLS-ALPN-01 challenges (must be reachable as port 443).
	#[arg(long, default_value_t = 443)]
	pub tunnel_alpn_port: u16,

	/// Allowed client domain suffixes (e.g. "example.com"). Clients whose domain
	/// does not end with one of these suffixes are rejected. May be specified multiple times.
	#[arg(long)]
	pub tunnel_domain_suffixes: Vec<String>,

	/// Path to PEM certificate chain. When --tunnel-acme-domain is set this is where the
	/// provisioned cert is written/read; without it the cert is used as-is with no auto-renewal.
	#[arg(long)]
	pub tunnel_cert_path: Option<String>,

	/// Path to PEM private key matching --tunnel-cert-path.
	#[arg(long)]
	pub tunnel_key_path: Option<String>,

	/// Server domain for ACME TLS-ALPN-01 provisioning (e.g. "relay.example.com").
	/// When set, the cert at --tunnel-cert-path is server-managed and auto-renewed.
	#[arg(long)]
	pub tunnel_acme_domain: Option<String>,

	/// Contact email for ACME account registration.
	#[arg(long)]
	pub tunnel_acme_email: Option<String>,

	/// Path to persist ACME account credentials.
	#[arg(long, default_value = "server_acme_creds.json")]
	pub tunnel_acme_creds_path: String,

	/// Use Let's Encrypt staging environment (for testing).
	#[arg(long)]
	pub tunnel_acme_staging: bool,

	/// Renew the server ACME cert this many days before expiry.
	#[arg(long, default_value_t = 30)]
	pub tunnel_acme_renew_days: u32,
}

#[derive(Debug, clap::Parser)]
#[group(skip)]
pub struct RunCmd {
	#[command(flatten)]
	pub run: cumulus_client_cli::RunCmd,

	/// Id of the parachain this collator collates for.
	#[clap(long)]
	pub parachain_id: Option<u32>,

	/// Maximum duration in milliseconds to produce a block
	#[clap(long, default_value = "2000", value_parser=block_authoring_duration_parser)]
	pub block_authoring_duration: Duration,

	#[command(flatten)]
	pub tunnel: TunnelArgs,
}

fn block_authoring_duration_parser(s: &str) -> Result<Duration, String> {
	Ok(Duration::from_millis(clap_num::number_range(s, 250, 2_000)?))
}

impl std::ops::Deref for RunCmd {
	type Target = cumulus_client_cli::RunCmd;

	fn deref(&self) -> &Self::Target {
		&self.run
	}
}

#[derive(Debug)]
pub struct RelayChainCli {
	/// The actual relay chain cli object.
	pub base: polkadot_cli::RunCmd,

	/// Optional chain id that should be passed to the relay chain.
	pub chain_id: Option<String>,

	/// The base path that should be used by the relay chain.
	pub base_path: Option<PathBuf>,
}

impl RelayChainCli {
	/// Parse the relay chain CLI parameters using the para chain `Configuration`.
	pub fn new<'a>(
		para_config: &sc_service::Configuration,
		relay_chain_args: impl Iterator<Item = &'a String>,
	) -> Self {
		let extension = crate::chain_spec::Extensions::try_get(&*para_config.chain_spec);
		let chain_id = extension.map(|e| e.relay_chain.clone());
		let base_path = para_config.base_path.path().join("polkadot");
		Self {
			base_path: Some(base_path),
			chain_id,
			base: clap::Parser::parse_from(relay_chain_args),
		}
	}
}
