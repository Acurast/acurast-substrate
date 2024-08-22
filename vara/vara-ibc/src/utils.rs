use core::fmt::Debug;
use gstd::{ext, format, Decode, Encode, TypeInfo};
use sails_rs::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, TypeInfo)]
pub enum IbcError {
	NotOwner,
	ContractPaused,
	MessageWithSameNoncePending,
	TTLSmallerThanMinimum,
	NotEnoughSignaturesValid,
	MessageNotFound,
	DeliveryConfirmationOverdue,
	SignatureInvalid,
	DuplicateSignature,
	PublicKeyUnknown,
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

pub fn secp256k1_ecdsa_recover_compressed(
	sig: &[u8; 65],
	msg: &[u8; 32],
) -> Result<[u8; 33], IbcError> {
	let rid = libsecp256k1::RecoveryId::parse(if sig[64] > 26 { sig[64] - 27 } else { sig[64] })
		.map_err(|_| IbcError::SignatureInvalid)?;
	let sig = libsecp256k1::Signature::parse_overflowing_slice(&sig[0..64])
		.map_err(|_| IbcError::SignatureInvalid)?;
	let msg = libsecp256k1::Message::parse(msg);
	let pubkey = libsecp256k1::recover(&msg, &sig, &rid).map_err(|_| IbcError::SignatureInvalid)?;
	Ok(pubkey.serialize_compressed())
}
