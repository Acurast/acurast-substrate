use parity_scale_codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};

use crate::application_crypto::p256::{Public, Signature};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_core::{crypto::UncheckedFrom, ecdsa, ed25519, sr25519, ConstU32, RuntimeDebug, H256};
use sp_runtime::{
	traits::{IdentifyAccount, Lazy, Verify},
	AccountId32, BoundedVec, MultiSignature as SPMultiSignature, MultiSigner as SPMultiSigner,
};
use sp_std::prelude::*;

pub type AuthenticatorData = BoundedVec<u8, ConstU32<37>>;
pub type ClientDataContext = (BoundedVec<u8, ConstU32<500>>, BoundedVec<u8, ConstU32<500>>);
pub type MessagePrefix = BoundedVec<u8, ConstU32<100>>;

const ACURAST_SIGNATURE_PREFIX: &[u8] = b"ACURAST TRANSACTION:\n";

#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(
	Eq,
	PartialEq,
	Clone,
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	RuntimeDebug,
	TypeInfo,
)]
pub enum MultiSignature {
	/// An Ed25519 signature.
	Ed25519(ed25519::Signature),
	/// An Sr25519 signature.
	Sr25519(sr25519::Signature),
	/// An ECDSA/SECP256k1 signature.
	Ecdsa(ecdsa::Signature),
	/// An ECDSA/SECP256r1 signature
	P256(Signature),
	/// An ECDSA/SECP256r1 signature with additional authenticator data
	P256WithAuthData(Signature, AuthenticatorData, Option<ClientDataContext>),
	/// An Ed25519 signature with message prefix.
	Ed25519WithPrefix(ed25519::Signature, MessagePrefix),
	/// An ECDSA/SECP256k1 signature with message prefix.
	K256WithPrefix(ecdsa::Signature, MessagePrefix),
	/// An Ed25519 signature with message prefix and Base64 encoding.
	Ed25519WithBase64(ed25519::Signature, MessagePrefix),
}

impl From<ed25519::Signature> for MultiSignature {
	fn from(x: ed25519::Signature) -> Self {
		Self::Ed25519(x)
	}
}

impl TryFrom<MultiSignature> for ed25519::Signature {
	type Error = ();
	fn try_from(m: MultiSignature) -> Result<Self, Self::Error> {
		if let MultiSignature::Ed25519(x) = m {
			Ok(x)
		} else {
			Err(())
		}
	}
}

impl From<sr25519::Signature> for MultiSignature {
	fn from(x: sr25519::Signature) -> Self {
		Self::Sr25519(x)
	}
}

impl TryFrom<MultiSignature> for sr25519::Signature {
	type Error = ();
	fn try_from(m: MultiSignature) -> Result<Self, Self::Error> {
		if let MultiSignature::Sr25519(x) = m {
			Ok(x)
		} else {
			Err(())
		}
	}
}

impl From<ecdsa::Signature> for MultiSignature {
	fn from(x: ecdsa::Signature) -> Self {
		Self::Ecdsa(x)
	}
}

impl TryFrom<MultiSignature> for ecdsa::Signature {
	type Error = ();
	fn try_from(m: MultiSignature) -> Result<Self, Self::Error> {
		if let MultiSignature::Ecdsa(x) = m {
			Ok(x)
		} else {
			Err(())
		}
	}
}

impl From<Signature> for MultiSignature {
	fn from(x: Signature) -> Self {
		Self::P256(x)
	}
}

impl TryFrom<MultiSignature> for Signature {
	type Error = ();
	fn try_from(m: MultiSignature) -> Result<Self, Self::Error> {
		if let MultiSignature::P256(x) = m {
			Ok(x)
		} else {
			Err(())
		}
	}
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum MultiSigner {
	/// An Ed25519 identity.
	Ed25519(ed25519::Public),
	/// An Sr25519 identity.
	Sr25519(sr25519::Public),
	/// An SECP256k1/ECDSA identity (actually, the Blake2 hash of the compressed pub key).
	Ecdsa(ecdsa::Public),
	/// An P256/ECDSA identity.
	P256(Public),
}

/// NOTE: This implementations is required by `SimpleAddressDeterminer`,
/// we convert the hash into some AccountId, it's fine to use any scheme.
impl<T: Into<H256>> UncheckedFrom<T> for MultiSigner {
	fn unchecked_from(x: T) -> Self {
		ed25519::Public::unchecked_from(x.into()).into()
	}
}

impl AsRef<[u8]> for MultiSigner {
	fn as_ref(&self) -> &[u8] {
		match self {
			Self::Ed25519(ref who) => who.as_ref(),
			Self::Sr25519(ref who) => who.as_ref(),
			Self::Ecdsa(ref who) => who.as_ref(),
			Self::P256(ref who) => who.as_ref(),
		}
	}
}

impl IdentifyAccount for MultiSigner {
	type AccountId = AccountId32;

	fn into_account(self) -> AccountId32 {
		match self {
			Self::Ed25519(who) => {
				let msigner: SPMultiSigner = who.into();
				msigner.into_account()
			},
			Self::Sr25519(who) => {
				let msigner: SPMultiSigner = who.into();
				msigner.into_account()
			},
			Self::Ecdsa(who) => {
				let msigner: SPMultiSigner = who.into();
				msigner.into_account()
			},
			Self::P256(who) => sp_io::hashing::blake2_256(who.as_ref()).into(),
		}
	}
}

impl From<ed25519::Public> for MultiSigner {
	fn from(x: ed25519::Public) -> Self {
		Self::Ed25519(x)
	}
}

impl TryFrom<MultiSigner> for ed25519::Public {
	type Error = ();
	fn try_from(m: MultiSigner) -> Result<Self, Self::Error> {
		if let MultiSigner::Ed25519(x) = m {
			Ok(x)
		} else {
			Err(())
		}
	}
}

impl From<sr25519::Public> for MultiSigner {
	fn from(x: sr25519::Public) -> Self {
		Self::Sr25519(x)
	}
}

impl TryFrom<MultiSigner> for sr25519::Public {
	type Error = ();
	fn try_from(m: MultiSigner) -> Result<Self, Self::Error> {
		if let MultiSigner::Sr25519(x) = m {
			Ok(x)
		} else {
			Err(())
		}
	}
}

impl From<ecdsa::Public> for MultiSigner {
	fn from(x: ecdsa::Public) -> Self {
		Self::Ecdsa(x)
	}
}

impl TryFrom<MultiSigner> for ecdsa::Public {
	type Error = ();
	fn try_from(m: MultiSigner) -> Result<Self, Self::Error> {
		if let MultiSigner::Ecdsa(x) = m {
			Ok(x)
		} else {
			Err(())
		}
	}
}

impl From<Public> for MultiSigner {
	fn from(x: Public) -> Self {
		Self::P256(x)
	}
}

impl TryFrom<MultiSigner> for Public {
	type Error = ();
	fn try_from(m: MultiSigner) -> Result<Self, Self::Error> {
		if let MultiSigner::P256(x) = m {
			Ok(x)
		} else {
			Err(())
		}
	}
}

#[cfg(feature = "std")]
impl std::fmt::Display for MultiSigner {
	fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
		match self {
			Self::Ed25519(ref who) => write!(fmt, "ed25519: {}", who),
			Self::Sr25519(ref who) => write!(fmt, "sr25519: {}", who),
			Self::Ecdsa(ref who) => write!(fmt, "ecdsa: {}", who),
			Self::P256(ref who) => write!(fmt, "p256: {}", who),
		}
	}
}

impl Verify for MultiSignature {
	type Signer = MultiSigner;

	fn verify<L: Lazy<[u8]>>(&self, mut msg: L, signer: &AccountId32) -> bool {
		match (self, signer) {
			(Self::Ed25519(multi_sig), _) => {
				let msig: SPMultiSignature = (*multi_sig).into();
				msig.verify(msg, signer)
			},
			(Self::Sr25519(multi_sig), _) => {
				let msig: SPMultiSignature = (*multi_sig).into();
				msig.verify(msg, signer)
			},
			(Self::Ecdsa(multi_sig), _) => {
				let msig: SPMultiSignature = (*multi_sig).into();
				msig.verify(msg, signer)
			},
			(Self::P256(ref sig), who) => {
				p256::ecdsa::recoverable::Signature::try_from(sig.as_ref())
					.and_then(|signature| signature.recover_verifying_key(msg.get()))
					.map(|pubkey| {
						&sp_io::hashing::blake2_256(pubkey.to_bytes().as_slice())
							== <dyn AsRef<[u8; 32]>>::as_ref(who)
					})
					.unwrap_or(false)
			},
			(Self::P256WithAuthData(sig, auth_data, client_context), who) => {
				p256::ecdsa::recoverable::Signature::try_from(sig.as_ref())
					.and_then(|signature| {
						let msg_bytes = msg.get();
						let signed_msg =
							Self::construct_full_message(msg_bytes, auth_data, client_context);
						signature.recover_verifying_key(&signed_msg)
					})
					.map(|pubkey| {
						&sp_io::hashing::blake2_256(pubkey.to_bytes().as_slice())
							== <dyn AsRef<[u8; 32]>>::as_ref(who)
					})
					.unwrap_or(false)
			},
			(Self::Ed25519WithPrefix(sig, prefix), _) => {
				let msig: SPMultiSignature = (*sig).into();
				let message = sp_io::hashing::blake2_256(
					&[prefix.as_slice(), ACURAST_SIGNATURE_PREFIX, msg.get()].concat(),
				);
				msig.verify(&message[..], signer)
			},
			(MultiSignature::K256WithPrefix(sig, prefix), who) => {
				let message = sp_io::hashing::keccak_256(
					&[prefix.as_slice(), ACURAST_SIGNATURE_PREFIX, msg.get()].concat(),
				);
				match sp_io::crypto::secp256k1_ecdsa_recover_compressed(sig.as_ref(), &message) {
					Ok(pubkey) => {
						&sp_io::hashing::blake2_256(pubkey.as_ref())
							== <dyn AsRef<[u8; 32]>>::as_ref(who)
					},
					_ => false,
				}
			},
			(Self::Ed25519WithBase64(sig, prefix), _) => {
				use base64::prelude::*;
				let encoded = BASE64_STANDARD.encode(msg.get()).into_bytes();
				let message =
					[prefix.as_slice(), ACURAST_SIGNATURE_PREFIX, encoded.as_slice()].concat();
				let msig: SPMultiSignature = (*sig).into();
				msig.verify(&message[..], signer)
			},
		}
	}
}

use base64::{
	alphabet,
	engine::{self, general_purpose},
	Engine as _,
};
const ENGINE: engine::GeneralPurpose =
	engine::GeneralPurpose::new(&alphabet::URL_SAFE, general_purpose::NO_PAD);

impl MultiSignature {
	fn construct_full_message(
		message: &[u8],
		auth_data: &AuthenticatorData,
		client_context: &Option<ClientDataContext>,
	) -> Vec<u8> {
		let msg = if message.len() != 32 {
			sp_io::hashing::sha2_256(message)
		} else {
			message.try_into().unwrap()
		};
		if let Some(client_context) = client_context {
			let encoded_message = ENGINE.encode(msg.as_slice());
			let client_data = [
				client_context.0.as_slice(),
				encoded_message.as_bytes(),
				client_context.1.as_slice(),
			]
			.concat();
			let msg = sp_io::hashing::sha2_256(&client_data);
			[auth_data.as_slice(), &msg].concat()
		} else {
			sp_io::hashing::sha2_256(&[auth_data.as_slice(), &msg].concat()).to_vec()
		}
	}
}
