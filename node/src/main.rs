//! Acurast Node CLI

#![warn(missing_docs)]

mod chain_spec;
mod cli;
mod client;
mod command;
mod rpc;
mod service;

fn main() -> sc_cli::Result<()> {
	command::run()
}
