#![cfg_attr(not(feature = "std"), no_std)]

pub mod barrier;
pub mod constants;
// pub mod migrations;
pub mod check_nonce;
pub mod types;
pub mod utils;
pub mod weight;

extern crate alloc;

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core data structures.
pub mod opaque {
	use sp_runtime::{generic, traits::BlakeTwo256, OpaqueExtrinsic as UncheckedExtrinsic};

	use crate::types::BlockNumber;

	/// Opaque block header type.
	pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
	/// Opaque block type.
	pub type Block = generic::Block<Header, UncheckedExtrinsic>;
	/// Opaque block identifier type.
	pub type BlockId = generic::BlockId<Block>;
	/// A Block signed with a Justification
	pub type SignedBlock = generic::SignedBlock<Block>;
}
