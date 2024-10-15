use core::fmt::Debug;
use gstd::{ext, format, Decode, Encode, TypeInfo};
use sails_rs::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, TypeInfo)]
pub enum ConsumerError {
	NotOwner,
	UnauthorizedCaller,
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
