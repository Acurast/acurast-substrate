// #[cfg(feature = "full_crypto")]
// use sp_application_crypto::Pair as SPAppPair;
use sp_application_crypto::RuntimePublic;
use sp_runtime::traits::Verify;
use sp_runtime_interface::runtime_interface;
use sp_std::vec::Vec;

pub use sp_core::crypto::KeyTypeId;

pub use crate::core::secp256r1::*;

/// secp256_r1 key type
pub const SECP256_R1: KeyTypeId = KeyTypeId(*b"p256");

mod app {
	use sp_application_crypto::app_crypto;
	use sp_runtime::BoundToRuntimeAppPublic;

	use super::SECP256_R1;

	app_crypto!(super, SECP256_R1);

	impl BoundToRuntimeAppPublic for Public {
		type Public = Self;
	}
}

pub use app::{Public as AppPublic, Signature as AppSignature};

impl RuntimePublic for Public {
	type Signature = Signature;

	fn all(_key_type: KeyTypeId) -> Vec<Self> {
		Vec::new()
	}

	fn generate_pair(_key_type: KeyTypeId, seed: Option<Vec<u8>>) -> Self {
		Pair::generate_from_seed_bytes(&seed.expect("seed needs to be provided"))
			.expect("Pair generation")
			.get_public()
	}

	fn sign<M: AsRef<[u8]>>(&self, _key_type: KeyTypeId, _msg: &M) -> Option<Self::Signature> {
		None
	}

	fn verify<M: AsRef<[u8]>>(&self, msg: &M, signature: &Self::Signature) -> bool {
		self::crypto::verify(signature, msg.as_ref(), &self)
	}

	fn to_raw_vec(&self) -> Vec<u8> {
		sp_core::crypto::ByteArray::to_raw_vec(self)
	}
}

#[runtime_interface]
pub trait Crypto {
	fn verify(sig: &Signature, msg: &[u8], pubkey: &Public) -> bool {
		sig.verify(msg, pubkey)
	}
}
