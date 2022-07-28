pub use acurast_multi_signature::*;
pub use acurast_multi_signer::*;

mod acurast_multi_signature {

	use crate::secp256r1;
	use sp_application_crypto::ByteArray;
	use sp_runtime::{
		app_crypto::{ecdsa, ed25519, sr25519},
		MultiSignature,
	};

	/// Signature verify that can work with any known signature types..
	#[derive(
		Eq,
		PartialEq,
		Clone,
		codec::Encode,
		codec::Decode,
		codec::MaxEncodedLen,
		frame_support::RuntimeDebug,
		scale_info::TypeInfo,
	)]
	pub enum AcurastMultiSignature {
		Primitive(MultiSignature),
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

	impl sp_runtime::traits::Verify for AcurastMultiSignature {
		type Signer = super::AcurastMultiSigner;
		fn verify<L: sp_runtime::traits::Lazy<[u8]>>(
			&self,
			msg: L,
			signer: &sp_runtime::AccountId32,
		) -> bool {
			match self {
				Self::Primitive(ref p) => p.verify(msg, signer),
				Self::P256(ref sig) => match secp256r1::Public::from_slice(signer.as_ref()) {
					Ok(signer_converted) => sig.verify(msg, &signer_converted),
					Err(()) => false,
				},
			}
		}
	}
}

mod acurast_multi_signer {
	use sp_runtime::{
		app_crypto::{ecdsa, ed25519, sr25519},
		MultiSigner,
	};

	use crate::secp256r1;
	use sp_core::crypto::AccountId32;

	/// Public key for any known crypto algorithm.
	#[derive(
		Eq,
		PartialEq,
		Ord,
		PartialOrd,
		Clone,
		codec::Encode,
		codec::Decode,
		frame_support::RuntimeDebug,
		scale_info::TypeInfo,
	)]
	pub enum AcurastMultiSigner {
		Primitive(MultiSigner),
		P256(secp256r1::Public),
	}

	/// NOTE: This implementations is required by `SimpleAddressDeterminer`,
	/// we convert the hash into some AccountId, it's fine to use any scheme.
	impl<T: Into<sp_core::H256>> sp_core::crypto::UncheckedFrom<T> for AcurastMultiSigner {
		fn unchecked_from(x: T) -> Self {
			AcurastMultiSigner::P256(secp256r1::Public::unchecked_from(x.into()))
		}
	}

	impl AsRef<[u8]> for AcurastMultiSigner {
		fn as_ref(&self) -> &[u8] {
			match *self {
				Self::Primitive(ref p) => p.as_ref(),
				Self::P256(ref p) => p.as_ref(),
			}
		}
	}

	impl sp_runtime::traits::IdentifyAccount for AcurastMultiSigner {
		type AccountId = AccountId32;
		fn into_account(self) -> AccountId32 {
			match self {
				Self::Primitive(p) => p.into_account(),
				Self::P256(p) => sp_io::hashing::blake2_256(p.as_ref()).into(),
			}
		}
	}

	impl From<ed25519::Public> for AcurastMultiSigner {
		fn from(x: ed25519::Public) -> Self {
			Self::Primitive(MultiSigner::from(x))
		}
	}

	impl From<sr25519::Public> for AcurastMultiSigner {
		fn from(x: sr25519::Public) -> Self {
			Self::Primitive(MultiSigner::from(x))
		}
	}

	impl From<ecdsa::Public> for AcurastMultiSigner {
		fn from(x: ecdsa::Public) -> Self {
			Self::Primitive(MultiSigner::from(x))
		}
	}

	impl From<secp256r1::Public> for AcurastMultiSigner {
		fn from(x: secp256r1::Public) -> Self {
			Self::P256(x)
		}
	}

	#[cfg(feature = "std")]
	impl std::fmt::Display for AcurastMultiSigner {
		fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
			match *self {
				Self::Primitive(ref p) => p.fmt(fmt),
				Self::P256(ref p) => write!(fmt, "p256: {}", p),
			}
		}
	}
}
