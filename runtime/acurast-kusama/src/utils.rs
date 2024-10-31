use crate::*;

pub const fn deposit(items: u32, bytes: u32) -> Balance {
	items as Balance * 1 * UNIT * SUPPLY_FACTOR + (bytes as Balance) * STORAGE_BYTE_FEE
}

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
	NativeVersion { runtime_version: VERSION, can_author_with: Default::default() }
}
