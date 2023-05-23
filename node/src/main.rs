//! Acurast Node CLI

#![warn(missing_docs)]

mod chain_spec;
#[macro_use]
mod service;
mod block_verifier;
mod cli;
mod client;
mod command;
mod rpc;

fn main() -> sc_cli::Result<()> {
	command::run()
}
