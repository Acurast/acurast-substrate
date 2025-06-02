use frame_support::{pallet_prelude::*, sp_runtime};
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
pub use sp_core::ecdsa::{
	Public, Signature, PUBLIC_KEY_SERIALIZED_SIZE, SIGNATURE_SERIALIZED_SIZE,
};
use sp_core::H160;
use sp_std::prelude::*;

#[derive(
	Eq,
	PartialEq,
	Copy,
	Clone,
	Encode,
	Decode,
	DecodeWithMemTracking,
	TypeInfo,
	MaxEncodedLen,
	Default,
	PartialOrd,
	Ord,
)]
pub struct AccountId20(pub [u8; 20]);

impl_serde::impl_fixed_hash_serde!(AccountId20, 20);

#[cfg(feature = "std")]
impl std::fmt::Display for AccountId20 {
	//TODO This is a pretty quck-n-dirty implementation. Perhaps we should add
	// checksum casing here? I bet there is a crate for that.
	// Maybe this one https://github.com/miguelmota/rust-eth-checksum
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{:?}", self.0)
	}
}

impl core::fmt::Debug for AccountId20 {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		write!(f, "{:?}", H160(self.0))
	}
}

impl From<[u8; 20]> for AccountId20 {
	fn from(bytes: [u8; 20]) -> Self {
		Self(bytes)
	}
}

impl From<AccountId20> for [u8; 20] {
	fn from(value: AccountId20) -> Self {
		value.0
	}
}

// NOTE: the implementation is lossy, and is intended to be used
// only to convert from Polkadot accounts to AccountId20.
// See https://github.com/moonbeam-foundation/moonbeam/pull/2315#discussion_r1205830577
// DO NOT USE IT FOR ANYTHING ELSE.
impl From<[u8; 32]> for AccountId20 {
	fn from(bytes: [u8; 32]) -> Self {
		let mut buffer = [0u8; 20];
		buffer.copy_from_slice(&bytes[..20]);
		Self(buffer)
	}
}
impl From<sp_runtime::AccountId32> for AccountId20 {
	fn from(account: sp_runtime::AccountId32) -> Self {
		let bytes: &[u8; 32] = account.as_ref();
		Self::from(*bytes)
	}
}

impl From<H160> for AccountId20 {
	fn from(h160: H160) -> Self {
		Self(h160.0)
	}
}

impl From<AccountId20> for H160 {
	fn from(value: AccountId20) -> Self {
		H160(value.0)
	}
}

#[cfg(feature = "std")]
impl std::str::FromStr for AccountId20 {
	type Err = &'static str;
	fn from_str(input: &str) -> Result<Self, Self::Err> {
		H160::from_str(input).map(Into::into).map_err(|_| "invalid hex address.")
	}
}
