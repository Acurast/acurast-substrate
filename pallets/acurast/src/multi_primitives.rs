pub use acurast_multi_signature::*;

mod acurast_multi_signature {

	use sp_runtime::{
		app_crypto::{sr25519, ed25519, ecdsa},
		MultiSignature,
	};
	use crate::secp256r1;

	/// Signature verify that can work with any known signature types..
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	#[derive(Eq, PartialEq, Clone, Encode, Decode, MaxEncodedLen, RuntimeDebug, TypeInfo)]
	pub enum AcurastMultiSignature {
		/// An Ed25519 signature.
		Primitive(MultiSignature),
		/// An Sr25519 signature.
		P256(secp256r1::Signature),
	}

	impl From<ed25519::Signature> for AcurastMultiSignature {
		fn from(x: ed25519::Signature) -> Self {
			Self::Primitive(MultiSignature::from(x))
		}
	}

	impl From<sr25519::Signature> for AcurastMultiSignature {
		fn from(x: sr25519::Signature) -> Self {
			Self::Primitive(MultiSignature::from(x))
		}
	}

	impl From<ecdsa::Signature> for AcurastMultiSignature {
		fn from(x: ecdsa::Signature) -> Self {
			Self::Primitive(MultiSignature::from(x))
		}
	}
	
	impl From<secp256r1::Signature> for AcurastMultiSignature {
		fn from(x: secp256r1::Signature) -> Self {
			Self::P256(x)
		}
	}
}


mod acurast_multi_signer {
	use sp_runtime::{
		app_crypto::{sr25519, ed25519, ecdsa},
		MultiSigner,
	};
	use crate::secp256r1;

	/// Public key for any known crypto algorithm.
	#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, codec::Encode, codec::Decode, frame_support::RuntimeDebug, scale_info::TypeInfo)]
	pub enum AcurastMultiSigner {
		Primitive(MultiSigner),
		P256(secp256r1::Public)
	}

}