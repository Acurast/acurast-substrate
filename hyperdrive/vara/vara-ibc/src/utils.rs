use core::fmt::Debug;
use gstd::{ext, format, Decode, Encode, TypeInfo};
use sails_rs::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, TypeInfo)]
pub enum IbcError {
	ContractPaused,
	NotOwner,
	IncorrectRecipient,
	MessageAlreadyReceived,
	PublicKeyUnknown,
	DuplicateSignature,
	SignatureInvalid,
	SignatureInvalidRecoveryId,
	SignatureInvalidRecoveredPubKeyLength,
	NotEnoughSignaturesValid,
	MessageWithSameNoncePending,
	TTLSmallerThanMinimum,
	MessageNotFound,
	DeliveryConfirmationOverdue,
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
	// e.g. 46c05b6368a44b8810d79859441d819b8e7cdc8bfd371e35c53196f4bcacdb5135c7facce2a97b95eacba8a586d87b7958aaf8368ab29cee481f76e871dbd9cb
	let signature =
		k256::ecdsa::Signature::from_slice(&sig[..64]).map_err(|_| IbcError::SignatureInvalid)?;

	let recid =
		k256::ecdsa::RecoveryId::try_from(if sig[64] > 26 { sig[64] - 27 } else { sig[64] })
			.map_err(|_| IbcError::SignatureInvalidRecoveryId)?;

	let recovered_key = k256::ecdsa::VerifyingKey::recover_from_prehash(msg, &signature, recid)
		.map_err(|_| IbcError::SignatureInvalid)?;

	let a: Box<[u8; 33]> = recovered_key
		.to_sec1_bytes()
		.try_into()
		.map_err(|_| IbcError::SignatureInvalidRecoveredPubKeyLength)?;
	Ok(*a)
}
