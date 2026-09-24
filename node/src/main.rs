//! Acurast Node CLI

#![warn(missing_docs)]
// `sc_cli::Result`'s error variant is large, and every entry point here returns it. Boxing it
// would churn signatures across the node for no gain on a binary's top-level error path.
#![allow(clippy::result_large_err)]

mod chain_spec;
mod cli;
mod client;
mod command;
mod rpc;
mod service;

fn main() -> sc_cli::Result<()> {
	let _ = rustls::crypto::ring::default_provider().install_default();
	command::run()
}
