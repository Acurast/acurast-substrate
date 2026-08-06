pub mod p256 {
	use sp_application_crypto::RuntimePublic;
	use sp_core::crypto::KeyTypeId;
	use sp_runtime::traits::Verify;
	use sp_std::prelude::*;

	pub use crate::core::p256::*;

	pub const P256: KeyTypeId = KeyTypeId(*b"p256");

	mod app {
		use sp_application_crypto::app_crypto;

		use super::P256;

		app_crypto!(super, P256);
	}

	pub use app::{Public as AppPublic, Signature as AppSignature};

	impl RuntimePublic for Public {
		type Signature = Signature;
		type ProofOfPossession = Signature;

		fn all(_key_type: KeyTypeId) -> Vec<Self> {
			vec![]
		}

		fn generate_pair(_key_type: KeyTypeId, seed: Option<Vec<u8>>) -> Self {
			Pair::generate_from_seed_bytes(&seed.expect("seed needs to be provided"))
				.expect("Pair generation")
				.get_public()
		}

		fn sign<M: AsRef<[u8]>>(&self, _key_type: KeyTypeId, _msg: &M) -> Option<Self::Signature> {
			// NOTE: this method cannot be implemented because we do not have access to the private key. Substrate implements this by accessing the CryptoStore,
			// we cannot access it and the current implementation does not support P256 keys. Also for our use case this is not really a problem since processors'
			// P256 private keys are not supposed to be outside the processor device.
			None
		}

		fn verify<M: AsRef<[u8]>>(&self, msg: &M, signature: &Self::Signature) -> bool {
			signature.verify(msg.as_ref(), self)
		}

		fn generate_proof_of_possession(
			&mut self,
			_key_type: KeyTypeId,
			_owner: &[u8],
		) -> Option<Self::ProofOfPossession> {
			// NOTE: cannot be implemented for the same reason as `sign`: we do not have access
			// to the private key (P256 private keys never leave the processor device).
			None
		}

		fn verify_proof_of_possession(
			&self,
			owner: &[u8],
			proof_of_possession: &Self::ProofOfPossession,
		) -> bool {
			use sp_core::proof_of_possession::ProofOfPossessionVerifier;
			Pair::verify_proof_of_possession(owner, proof_of_possession, self)
		}

		fn to_raw_vec(&self) -> Vec<u8> {
			sp_core::crypto::ByteArray::to_raw_vec(self)
		}
	}
}
