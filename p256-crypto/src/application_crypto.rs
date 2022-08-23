pub mod p256 {
	use sp_application_crypto::RuntimePublic;
	use sp_core::crypto::KeyTypeId;
	use sp_runtime::traits::Verify;
	use sp_std::prelude::*;

	pub use crate::core::p256::*;

	pub const P256: KeyTypeId = KeyTypeId(*b"p256");

	mod app {
		use sp_application_crypto::app_crypto;
		use sp_runtime::BoundToRuntimeAppPublic;

		use crate::core::p256;

		use super::P256;

		app_crypto!(p256, P256);

		impl BoundToRuntimeAppPublic for Public {
			type Public = Self;
		}
	}

	pub use app::{Public as AppPublic, Signature as AppSignature};

	impl RuntimePublic for Public {
		type Signature = Signature;

		fn all(_key_type: KeyTypeId) -> Vec<Self> {
			vec![]
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
			signature.verify(msg.as_ref(), &self)
		}

		fn to_raw_vec(&self) -> Vec<u8> {
			sp_core::crypto::ByteArray::to_raw_vec(self)
		}
	}
}
