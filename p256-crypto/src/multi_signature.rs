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
	/// An ECDSA/SECP256k1 signature with message prefix and EIP-712 replay-safe hash.
	/// For smart wallets (ERC-4337) that wrap personal_sign in an EIP-712 typed hash.
	/// Contains: signature, message prefix, precomputed domain separator, message type hash.
	K256WithPrefixEIP712(ecdsa::Signature, MessagePrefix, [u8; 32], [u8; 32]),
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
			(MultiSignature::K256WithPrefixEIP712(sig, prefix, domain_separator, message_type_hash), who) => {
				// CBS smart wallets wrap personal_sign in a single EIP-712 replay-safe hash:
				//   1. Wallet computes standard_hash = keccak256(eth_prefix || message)
				//   2. Wallet wraps: signed = eip712Hash(standard_hash)
				let original_hash = sp_io::hashing::keccak_256(
					&[prefix.as_slice(), ACURAST_SIGNATURE_PREFIX, msg.get()].concat(),
				);
				let replay_safe_hash = Self::compute_eip712_hash(
					&original_hash,
					domain_separator,
					message_type_hash,
				);
				match sp_io::crypto::secp256k1_ecdsa_recover_compressed(
					sig.as_ref(),
					&replay_safe_hash,
				) {
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
	fn compute_eip712_hash(
		original_hash: &[u8; 32],
		domain_separator: &[u8; 32],
		message_type_hash: &[u8; 32],
	) -> [u8; 32] {
		// struct_hash = keccak256(message_type_hash || original_hash)
		let mut struct_buf = [0u8; 64];
		struct_buf[0..32].copy_from_slice(message_type_hash);
		struct_buf[32..64].copy_from_slice(original_hash);
		let struct_hash = sp_io::hashing::keccak_256(&struct_buf);

		// keccak256("\x19\x01" || domain_separator || struct_hash)
		let mut final_buf = [0u8; 66];
		final_buf[0] = 0x19;
		final_buf[1] = 0x01;
		final_buf[2..34].copy_from_slice(domain_separator);
		final_buf[34..66].copy_from_slice(&struct_hash);
		sp_io::hashing::keccak_256(&final_buf)
	}

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

#[cfg(test)]
mod tests {
	use super::*;
	use hex_literal::hex;
	use parity_scale_codec::{Decode, Encode};
	use sp_core::Pair;
	use sp_runtime::traits::Verify;

	#[test]
	fn eip712_hash_computation() {
		let original_hash: [u8; 32] =
			hex!("abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd");
		let domain_separator: [u8; 32] =
			hex!("1111111111111111111111111111111111111111111111111111111111111111");
		let message_type_hash: [u8; 32] =
			hex!("2222222222222222222222222222222222222222222222222222222222222222");

		let result = MultiSignature::compute_eip712_hash(
			&original_hash,
			&domain_separator,
			&message_type_hash,
		);

		// Manually compute expected value
		let mut struct_buf = [0u8; 64];
		struct_buf[0..32].copy_from_slice(&message_type_hash);
		struct_buf[32..64].copy_from_slice(&original_hash);
		let struct_hash = sp_io::hashing::keccak_256(&struct_buf);
		let mut final_buf = [0u8; 66];
		final_buf[0] = 0x19;
		final_buf[1] = 0x01;
		final_buf[2..34].copy_from_slice(&domain_separator);
		final_buf[34..66].copy_from_slice(&struct_hash);
		let expected = sp_io::hashing::keccak_256(&final_buf);

		assert_eq!(result, expected);
	}

	#[test]
	fn eip712_signature_verification() {
		// Use Coinbase Smart Wallet constants as a concrete example.
		// CBS wraps personal_sign in a single EIP-712 replay-safe hash.
		let cbs_message_type_hash =
			sp_io::hashing::keccak_256(b"CoinbaseSmartWalletMessage(bytes32 hash)");

		let domain_type_hash = sp_io::hashing::keccak_256(
			b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
		);
		let name_hash = sp_io::hashing::keccak_256(b"Coinbase Smart Wallet");
		let version_hash = sp_io::hashing::keccak_256(b"1");
		let chain_id: u64 = 8453;
		let wallet_address: [u8; 20] = hex!("1234567890abcdef1234567890abcdef12345678");

		let mut domain_buf = [0u8; 160];
		domain_buf[0..32].copy_from_slice(&domain_type_hash);
		domain_buf[32..64].copy_from_slice(&name_hash);
		domain_buf[64..96].copy_from_slice(&version_hash);
		domain_buf[120..128].copy_from_slice(&chain_id.to_be_bytes());
		domain_buf[140..160].copy_from_slice(&wallet_address);
		let domain_separator = sp_io::hashing::keccak_256(&domain_buf);

		let prefix: MessagePrefix =
			b"\x19Ethereum Signed Message:\n32".to_vec().try_into().unwrap();
		let tx_payload = b"test transaction payload";

		// Step 1: original_hash (same as personal_sign)
		let original_hash = sp_io::hashing::keccak_256(
			&[prefix.as_slice(), ACURAST_SIGNATURE_PREFIX, tx_payload.as_slice()].concat(),
		);

		// Step 2: Single EIP-712 wrap (CBS replay-safe hash for personal_sign)
		let replay_safe_hash = MultiSignature::compute_eip712_hash(
			&original_hash,
			&domain_separator,
			&cbs_message_type_hash,
		);

		// Step 3: Sign the replay-safe hash
		let (pair, _) = sp_core::ecdsa::Pair::generate();
		let sig = pair.sign_prehashed(&replay_safe_hash);

		let account_id: AccountId32 =
			sp_io::hashing::blake2_256(pair.public().as_ref()).into();

		let multi_sig = MultiSignature::K256WithPrefixEIP712(
			sig,
			prefix,
			domain_separator,
			cbs_message_type_hash,
		);
		assert!(multi_sig.verify(&tx_payload[..], &account_id));
	}

	#[test]
	fn eip712_wrong_domain_separator_fails() {
		let message_type_hash: [u8; 32] =
			sp_io::hashing::keccak_256(b"CoinbaseSmartWalletMessage(bytes32 hash)");
		let domain_separator: [u8; 32] =
			hex!("1111111111111111111111111111111111111111111111111111111111111111");
		let wrong_domain_separator: [u8; 32] =
			hex!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
		let prefix: MessagePrefix =
			b"\x19Ethereum Signed Message:\n32".to_vec().try_into().unwrap();
		let tx_payload = b"test transaction payload";

		let original_hash = sp_io::hashing::keccak_256(
			&[prefix.as_slice(), ACURAST_SIGNATURE_PREFIX, tx_payload.as_slice()].concat(),
		);
		// Sign with correct domain (single-wrap)
		let replay_safe_hash = MultiSignature::compute_eip712_hash(&original_hash, &domain_separator, &message_type_hash);

		let (pair, _) = sp_core::ecdsa::Pair::generate();
		let sig = pair.sign_prehashed(&replay_safe_hash);
		let account_id: AccountId32 =
			sp_io::hashing::blake2_256(pair.public().as_ref()).into();

		// Verify with wrong domain_separator should fail
		let multi_sig = MultiSignature::K256WithPrefixEIP712(
			sig,
			prefix,
			wrong_domain_separator,
			message_type_hash,
		);
		assert!(!multi_sig.verify(&tx_payload[..], &account_id));
	}

	#[test]
	fn eip712_wrong_message_type_hash_fails() {
		let message_type_hash: [u8; 32] =
			sp_io::hashing::keccak_256(b"CoinbaseSmartWalletMessage(bytes32 hash)");
		let wrong_message_type_hash: [u8; 32] =
			sp_io::hashing::keccak_256(b"SafeMessage(bytes32 message)");
		let domain_separator: [u8; 32] =
			hex!("1111111111111111111111111111111111111111111111111111111111111111");
		let prefix: MessagePrefix =
			b"\x19Ethereum Signed Message:\n32".to_vec().try_into().unwrap();
		let tx_payload = b"test transaction payload";

		let original_hash = sp_io::hashing::keccak_256(
			&[prefix.as_slice(), ACURAST_SIGNATURE_PREFIX, tx_payload.as_slice()].concat(),
		);
		// Sign with correct type hash (single-wrap)
		let replay_safe_hash = MultiSignature::compute_eip712_hash(&original_hash, &domain_separator, &message_type_hash);

		let (pair, _) = sp_core::ecdsa::Pair::generate();
		let sig = pair.sign_prehashed(&replay_safe_hash);
		let account_id: AccountId32 =
			sp_io::hashing::blake2_256(pair.public().as_ref()).into();

		// Verify with wrong message_type_hash should fail
		let multi_sig = MultiSignature::K256WithPrefixEIP712(
			sig,
			prefix,
			domain_separator,
			wrong_message_type_hash,
		);
		assert!(!multi_sig.verify(&tx_payload[..], &account_id));
	}

	#[test]
	fn eip712_tampered_message_fails() {
		let message_type_hash: [u8; 32] =
			sp_io::hashing::keccak_256(b"CoinbaseSmartWalletMessage(bytes32 hash)");
		let domain_separator: [u8; 32] =
			hex!("1111111111111111111111111111111111111111111111111111111111111111");
		let prefix: MessagePrefix =
			b"\x19Ethereum Signed Message:\n32".to_vec().try_into().unwrap();
		let tx_payload = b"test transaction payload";
		let tampered_payload = b"tampered transaction payload";

		let original_hash = sp_io::hashing::keccak_256(
			&[prefix.as_slice(), ACURAST_SIGNATURE_PREFIX, tx_payload.as_slice()].concat(),
		);
		// Sign with correct message (single-wrap)
		let replay_safe_hash = MultiSignature::compute_eip712_hash(&original_hash, &domain_separator, &message_type_hash);

		let (pair, _) = sp_core::ecdsa::Pair::generate();
		let sig = pair.sign_prehashed(&replay_safe_hash);
		let account_id: AccountId32 =
			sp_io::hashing::blake2_256(pair.public().as_ref()).into();

		// Verify with tampered message should fail
		let multi_sig = MultiSignature::K256WithPrefixEIP712(
			sig,
			prefix,
			domain_separator,
			message_type_hash,
		);
		assert!(!multi_sig.verify(&tampered_payload[..], &account_id));
	}

	#[test]
	fn eip712_scale_codec_roundtrip() {
		let domain_separator: [u8; 32] =
			hex!("1111111111111111111111111111111111111111111111111111111111111111");
		let message_type_hash: [u8; 32] =
			hex!("2222222222222222222222222222222222222222222222222222222222222222");
		let prefix: MessagePrefix =
			b"\x19Ethereum Signed Message:\n32".to_vec().try_into().unwrap();
		let sig = ecdsa::Signature::from_raw([0u8; 65]);

		let multi_sig = MultiSignature::K256WithPrefixEIP712(
			sig,
			prefix,
			domain_separator,
			message_type_hash,
		);

		let encoded = multi_sig.encode();
		// Variant index should be 8 (0-indexed: Ed25519=0, Sr25519=1, Ecdsa=2, P256=3,
		// P256WithAuthData=4, Ed25519WithPrefix=5, K256WithPrefix=6, Ed25519WithBase64=7,
		// K256WithPrefixEIP712=8)
		assert_eq!(encoded[0], 8);

		let decoded = MultiSignature::decode(&mut &encoded[..]).unwrap();
		assert_eq!(multi_sig, decoded);
	}

	#[test]
	fn eip712_real_coinbase_smart_wallet_verification() {
		// Real data captured from a Coinbase Smart Wallet (Base) via the hub frontend.
		// ETH address: 0x9b9d94b745df3507e714ab9458cc2601532e104c
		//
		// Verifies that the runtime correctly recovers the public key from a real
		// CBS EIP-712 wrapped signature using domain_separator and message_type_hash
		// derived from the wallet's on-chain contract.

		let ecdsa_sig = hex!("4f5bf8b4b5c4d7ceeaf1b1500b596959b3b0e573fd49898123ac08231ebdc5ce05b06a9e92ea07abe908ddc1dc4b6842084be8c793ec51f30d338af70f3bc05f1b");
		let domain_separator = hex!("a22b2624ca4ecf1d8e1c1f292d5bb97e9304c40fc9d100e4adfd5e0cf6d40cf1");
		let message_type_hash = hex!("9b493d222105fee7df163ab5d57f0bf1ffd2da04dd5fafbe10b54c41c1adc657");
		let pub_key = hex!("02d77a21e3ba23b6c77cbe76fee6a57c024d3249f06f5c395398b052ad4a57cc44");

		// A login challenge was signed via personal_sign. The wallet hashes:
		// keccak256(eth_prefix || challenge), then wraps in EIP-712.
		// This test verifies end-to-end EIP-712 recovery with real Base Wallet data.

		let prefix: MessagePrefix =
			b"\x19Ethereum Signed Message:\n44".to_vec().try_into().unwrap();
		let challenge = b"Login to https://angular.lukeisontheroad.com";

		// The CBS wallet computes: keccak256(eth_prefix || challenge) as the standard hash,
		// then wraps it in EIP-712
		let standard_hash = sp_io::hashing::keccak_256(
			&[prefix.as_slice(), challenge.as_slice()].concat(),
		);
		let replay_safe_hash = MultiSignature::compute_eip712_hash(
			&standard_hash,
			&domain_separator,
			&message_type_hash,
		);

		// Verify recovery of the correct public key
		let mut sig_bytes = ecdsa_sig;
		sig_bytes[64] = 0; // v=27 -> recovery_id=0
		let recovered = sp_io::crypto::secp256k1_ecdsa_recover_compressed(
			&sig_bytes,
			&replay_safe_hash,
		);
		let recovered_pubkey = match recovered {
			Ok(pk) => pk,
			Err(_) => panic!("secp256k1 recovery failed"),
		};
		assert_eq!(recovered_pubkey, pub_key, "Recovered pubkey must match the stored CBS wallet key");

		// Verify account_id derivation
		let account_id: AccountId32 =
			sp_io::hashing::blake2_256(&pub_key).into();
		let recovered_account: AccountId32 =
			sp_io::hashing::blake2_256(&recovered_pubkey).into();
		assert_eq!(recovered_account, account_id);

		// Verify messageTypeHash is keccak256("CoinbaseSmartWalletMessage(bytes32 hash)")
		let expected_type_hash = sp_io::hashing::keccak_256(
			b"CoinbaseSmartWalletMessage(bytes32 hash)",
		);
		assert_eq!(message_type_hash, expected_type_hash);

		// Verify domain separator matches the CBS EIP-712 domain for this wallet address
		let domain_type_hash = sp_io::hashing::keccak_256(
			b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
		);
		let name_hash = sp_io::hashing::keccak_256(b"Coinbase Smart Wallet");
		let version_hash = sp_io::hashing::keccak_256(b"1");
		let mut domain_buf = [0u8; 160];
		domain_buf[0..32].copy_from_slice(&domain_type_hash);
		domain_buf[32..64].copy_from_slice(&name_hash);
		domain_buf[64..96].copy_from_slice(&version_hash);
		domain_buf[120..128].copy_from_slice(&8453u64.to_be_bytes()); // Base mainnet chainId
		let wallet_addr = hex!("9b9d94b745df3507e714ab9458cc2601532e104c");
		domain_buf[140..160].copy_from_slice(&wallet_addr);
		let expected_domain = sp_io::hashing::keccak_256(&domain_buf);
		assert_eq!(domain_separator, expected_domain);
	}

	#[test]
	fn eip712_real_cbs_transaction_hash_chain() {
		// Real transaction data captured from CBS smart wallet debug overlay.
		// Verifies the hash chain: extrinsic_payload -> original_hash -> EIP-712 wrap.
		//
		// Wallet: 0x9b9d94b745df3507e714ab9458cc2601532e104c
		// Same key as the login test above.
		//
		// NOTE: Signature recovery is NOT tested here — this test focuses on
		// verifying the hash chain is deterministic and correct.
		// The login test above proves end-to-end recovery works.

		let domain_separator = hex!("a22b2624ca4ecf1d8e1c1f292d5bb97e9304c40fc9d100e4adfd5e0cf6d40cf1");
		let message_type_hash = hex!("9b493d222105fee7df163ab5d57f0bf1ffd2da04dd5fafbe10b54c41c1adc657");

		// Transaction 1 rawData from debug overlay
		let extrinsic_payload = hex!("0a03002f0a90d4ec02324c97c1b0e42b5423368a6b1210d2210f6e5872a6779e439aae070010a5d4e8550300000a000000010000004b5f95eefedf0d0fb514339edc24d2d411310520f687b4146145bcedb99885b9d939372b134ba21a246f1bf2085624d535a1c792178b2a448e321cf7006d7800");

		// Prefix from debug overlay: "\x19Ethereum Signed Message:\n138"
		// 138 = 21 (ACURAST_SIGNATURE_PREFIX) + 117 (extrinsic payload bytes)
		let prefix: MessagePrefix =
			b"\x19Ethereum Signed Message:\n138".to_vec().try_into().unwrap();

		// Step 1: Verify original_hash matches captured debug value
		let original_hash = sp_io::hashing::keccak_256(
			&[prefix.as_slice(), ACURAST_SIGNATURE_PREFIX, &extrinsic_payload].concat(),
		);
		assert_eq!(
			original_hash,
			hex!("84d27bd84c3ce049d88435b4828f1d7154bec5291025e046ae35b80cf7b040e4"),
			"original_hash must match the value captured from CBS debug overlay"
		);

		// Step 2: Verify EIP-712 wrap produces a deterministic replay_safe_hash
		let replay_safe_hash = MultiSignature::compute_eip712_hash(
			&original_hash,
			&domain_separator,
			&message_type_hash,
		);
		// The replay_safe_hash is what the wallet WOULD sign if it produced a fresh signature.
		// Verify it's deterministic and non-zero.
		assert_ne!(replay_safe_hash, [0u8; 32]);
		assert_ne!(replay_safe_hash, original_hash,
			"EIP-712 wrap must produce a different hash than the original");

		// Step 3: Verify Transaction 2 produces a DIFFERENT hash (different payload)
		let extrinsic_payload_2 = hex!("0a03002f0a90d4ec02324c97c1b0e42b5423368a6b1210d2210f6e5872a6779e439aae070010a5d4e8750300000a000000010000004b5f95eefedf0d0fb514339edc24d2d411310520f687b4146145bcedb99885b9f85d293c877c83ce96a7f3dfb53062169ba267a7e92928be105bd28cdd88bb06");
		let original_hash_2 = sp_io::hashing::keccak_256(
			&[prefix.as_slice(), ACURAST_SIGNATURE_PREFIX, &extrinsic_payload_2].concat(),
		);
		assert_eq!(
			original_hash_2,
			hex!("fd12ab1833fd2ecdcfb3edea98f24559579fde0b6a999649a2fd8d25085e15af"),
			"Transaction 2 original_hash must match captured debug value"
		);

		let replay_safe_hash_2 = MultiSignature::compute_eip712_hash(
			&original_hash_2,
			&domain_separator,
			&message_type_hash,
		);
		assert_ne!(replay_safe_hash, replay_safe_hash_2,
			"Different transactions must produce different replay_safe_hashes");
	}

}
