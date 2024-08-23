use core::fmt::Debug;
use gstd::{ext, format, Decode, Encode, TypeInfo};
use sails_rs::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, TypeInfo)]
pub enum ProxyError {
	UnknownJobVersion(u16),
	JobAlreadyFinished,
	NotJobProcessor,
	UnknownJob,
	ContractPaused,
	NotOwner,
	NotJobCreator,
	CannotFinalizeJob,
	OutgoingActionTooBig,
	Verbose(String),
	InvalidIncomingAction(String),
	/// Error wrappers
	ConsumerError(String),
	LangError(String),
	IbcFailed,
}

pub fn panicking<T, E: Debug, F: FnOnce() -> Result<T, E>>(f: F) -> T {
	match f() {
		Ok(v) => v,
		Err(e) => panic(e),
	}
}

pub fn panic(err: impl Debug) -> ! {
	ext::panic(&format!("{err:?}"))
}

#[inline(always)]
fn blake2<const N: usize>(data: &[u8]) -> [u8; N] {
	blake2b_simd::Params::new()
		.hash_length(N)
		.hash(data)
		.as_bytes()
		.try_into()
		.expect("slice is always the necessary length")
}

pub fn blake2_256(data: &[u8]) -> [u8; 32] {
	blake2(data)
}
