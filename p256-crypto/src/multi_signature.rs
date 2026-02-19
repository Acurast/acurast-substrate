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
pub type WalletAddress = [u8; 20];

const ACURAST_SIGNATURE_PREFIX: &[u8] = b"ACURAST TRANSACTION:\n";

/// keccak256("EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)")
const CBS_DOMAIN_TYPE_HASH: [u8; 32] = [
	0x8b, 0x73, 0xc3, 0xc6, 0x9b, 0xb8, 0xfe, 0x3d, 0x51, 0x2e, 0xcc, 0x4c, 0xf7, 0x59, 0xcc,
	0x79, 0x23, 0x9f, 0x7b, 0x17, 0x9b, 0x0f, 0xfa, 0xca, 0xa9, 0xa7, 0x5d, 0x52, 0x2b, 0x39,
	0x40, 0x0f,
];
/// keccak256("CoinbaseSmartWalletMessage(bytes32 hash)")
const CBS_MESSAGE_TYPE_HASH: [u8; 32] = [
	0x9b, 0x49, 0x3d, 0x22, 0x21, 0x05, 0xfe, 0xe7, 0xdf, 0x16, 0x3a, 0xb5, 0xd5, 0x7f, 0x0b,
	0xf1, 0xff, 0xd2, 0xda, 0x04, 0xdd, 0x5f, 0xaf, 0xbe, 0x10, 0xb5, 0x4c, 0x41, 0xc1, 0xad,
	0xc6, 0x57,
];
/// keccak256("Coinbase Smart Wallet")
const CBS_NAME_HASH: [u8; 32] = [
	0x65, 0x34, 0x2b, 0x33, 0x65, 0x2b, 0x3c, 0x0b, 0xc1, 0x73, 0x04, 0x9e, 0xd7, 0x49, 0x6b,
	0x73, 0x22, 0xdb, 0x7f, 0xf8, 0xab, 0xfc, 0x07, 0x52, 0x04, 0x4a, 0x5a, 0x59, 0x2b, 0xdd,
	0xe6, 0x4e,
];
/// keccak256("1")
const CBS_VERSION_HASH: [u8; 32] = [
	0xc8, 0x9e, 0xfd, 0xaa, 0x54, 0xc0, 0xf2, 0x0c, 0x7a, 0xdf, 0x61, 0x28, 0x82, 0xdf, 0x09,
	0x50, 0xf5, 0xa9, 0x51, 0x63, 0x7e, 0x03, 0x07, 0xcd, 0xcb, 0x4c, 0x67, 0x2f, 0x29, 0x8b,
	0x8b, 0xc6,
];

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
	/// An ECDSA/SECP256k1 signature with message prefix for Coinbase Smart Wallet.
	/// Contains: signature, message prefix, wallet contract address (20 bytes), chain ID.
	K256WithPrefixCBS(ecdsa::Signature, MessagePrefix, WalletAddress, u64),
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
			(MultiSignature::K256WithPrefixCBS(sig, prefix, wallet_address, chain_id), who) => {
				let original_hash = sp_io::hashing::keccak_256(
					&[prefix.as_slice(), ACURAST_SIGNATURE_PREFIX, msg.get()].concat(),
				);
				let replay_safe_hash = Self::compute_cbs_replay_safe_hash(
					&original_hash,
					wallet_address,
					*chain_id,
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
	fn compute_cbs_domain_separator(wallet_address: &WalletAddress, chain_id: u64) -> [u8; 32] {
		let mut buf = [0u8; 160];
		buf[0..32].copy_from_slice(&CBS_DOMAIN_TYPE_HASH);
		buf[32..64].copy_from_slice(&CBS_NAME_HASH);
		buf[64..96].copy_from_slice(&CBS_VERSION_HASH);
		// chain_id as uint256: big-endian u64 at bytes 120..128
		buf[120..128].copy_from_slice(&chain_id.to_be_bytes());
		// address: 20 bytes right-aligned at bytes 140..160
		buf[140..160].copy_from_slice(wallet_address);
		sp_io::hashing::keccak_256(&buf)
	}

	fn compute_cbs_replay_safe_hash(
		original_hash: &[u8; 32],
		wallet_address: &WalletAddress,
		chain_id: u64,
	) -> [u8; 32] {
		let domain_separator = Self::compute_cbs_domain_separator(wallet_address, chain_id);

		// struct_hash = keccak256(CBS_MESSAGE_TYPE_HASH || original_hash)
		let mut struct_buf = [0u8; 64];
		struct_buf[0..32].copy_from_slice(&CBS_MESSAGE_TYPE_HASH);
		struct_buf[32..64].copy_from_slice(original_hash);
		let struct_hash = sp_io::hashing::keccak_256(&struct_buf);

		// EIP-712: keccak256("\x19\x01" || domain_separator || struct_hash)
		let mut final_buf = [0u8; 66];
		final_buf[0] = 0x19;
		final_buf[1] = 0x01;
		final_buf[2..34].copy_from_slice(&domain_separator);
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
	fn cbs_constant_domain_type_hash() {
		let computed = sp_io::hashing::keccak_256(
			b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
		);
		assert_eq!(computed, CBS_DOMAIN_TYPE_HASH);
	}

	#[test]
	fn cbs_constant_message_type_hash() {
		let computed =
			sp_io::hashing::keccak_256(b"CoinbaseSmartWalletMessage(bytes32 hash)");
		assert_eq!(computed, CBS_MESSAGE_TYPE_HASH);
	}

	#[test]
	fn cbs_constant_name_hash() {
		let computed = sp_io::hashing::keccak_256(b"Coinbase Smart Wallet");
		assert_eq!(computed, CBS_NAME_HASH);
	}

	#[test]
	fn cbs_constant_version_hash() {
		let computed = sp_io::hashing::keccak_256(b"1");
		assert_eq!(computed, CBS_VERSION_HASH);
	}

	#[test]
	fn cbs_domain_separator_computation() {
		// Compute domain separator for a known wallet address on Base (chainId=8453)
		let wallet_address: WalletAddress = hex!("1234567890abcdef1234567890abcdef12345678");
		let chain_id: u64 = 8453;

		let domain_sep =
			MultiSignature::compute_cbs_domain_separator(&wallet_address, chain_id);

		// Manually compute expected value
		let mut buf = [0u8; 160];
		buf[0..32].copy_from_slice(&CBS_DOMAIN_TYPE_HASH);
		buf[32..64].copy_from_slice(&CBS_NAME_HASH);
		buf[64..96].copy_from_slice(&CBS_VERSION_HASH);
		buf[120..128].copy_from_slice(&chain_id.to_be_bytes());
		buf[140..160].copy_from_slice(&wallet_address);
		let expected = sp_io::hashing::keccak_256(&buf);

		assert_eq!(domain_sep, expected);
	}

	#[test]
	fn cbs_replay_safe_hash_computation() {
		let original_hash: [u8; 32] =
			hex!("abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd");
		let wallet_address: WalletAddress = hex!("1234567890abcdef1234567890abcdef12345678");
		let chain_id: u64 = 8453;

		let replay_safe = MultiSignature::compute_cbs_replay_safe_hash(
			&original_hash,
			&wallet_address,
			chain_id,
		);

		// Manually compute expected
		let domain_sep =
			MultiSignature::compute_cbs_domain_separator(&wallet_address, chain_id);
		let mut struct_buf = [0u8; 64];
		struct_buf[0..32].copy_from_slice(&CBS_MESSAGE_TYPE_HASH);
		struct_buf[32..64].copy_from_slice(&original_hash);
		let struct_hash = sp_io::hashing::keccak_256(&struct_buf);
		let mut final_buf = [0u8; 66];
		final_buf[0] = 0x19;
		final_buf[1] = 0x01;
		final_buf[2..34].copy_from_slice(&domain_sep);
		final_buf[34..66].copy_from_slice(&struct_hash);
		let expected = sp_io::hashing::keccak_256(&final_buf);

		assert_eq!(replay_safe, expected);
	}

	#[test]
	fn cbs_signature_verification() {
		let wallet_address: WalletAddress = hex!("1234567890abcdef1234567890abcdef12345678");
		let chain_id: u64 = 8453;
		let prefix_bytes = b"\x19Ethereum Signed Message:\n32";
		let prefix: MessagePrefix = prefix_bytes.to_vec().try_into().unwrap();

		// The "transaction" payload
		let tx_payload = b"test transaction payload";

		// Step 1: Compute the original_hash (what K256WithPrefix would compute)
		let original_hash = sp_io::hashing::keccak_256(
			&[prefix.as_slice(), ACURAST_SIGNATURE_PREFIX, tx_payload.as_slice()].concat(),
		);

		// Step 2: Compute the replay-safe hash (what the CBS wallet wraps)
		let replay_safe_hash = MultiSignature::compute_cbs_replay_safe_hash(
			&original_hash,
			&wallet_address,
			chain_id,
		);

		// Step 3: Sign the replay-safe hash with a secp256k1 key
		let (pair, _) = sp_core::ecdsa::Pair::generate();
		let sig = pair.sign_prehashed(&replay_safe_hash);

		// Step 4: Compute the expected AccountId (blake2_256 of compressed pubkey)
		let account_id: AccountId32 =
			sp_io::hashing::blake2_256(pair.public().as_ref()).into();

		// Step 5: Construct MultiSignature and verify
		let multi_sig = MultiSignature::K256WithPrefixCBS(
			sig,
			prefix.clone(),
			wallet_address,
			chain_id,
		);
		assert!(multi_sig.verify(&tx_payload[..], &account_id));
	}

	#[test]
	fn cbs_wrong_chain_id_fails() {
		let wallet_address: WalletAddress = hex!("1234567890abcdef1234567890abcdef12345678");
		let chain_id: u64 = 8453;
		let wrong_chain_id: u64 = 1; // Ethereum mainnet
		let prefix: MessagePrefix = b"\x19Ethereum Signed Message:\n32".to_vec().try_into().unwrap();
		let tx_payload = b"test transaction payload";

		let original_hash = sp_io::hashing::keccak_256(
			&[prefix.as_slice(), ACURAST_SIGNATURE_PREFIX, tx_payload.as_slice()].concat(),
		);
		let replay_safe_hash = MultiSignature::compute_cbs_replay_safe_hash(
			&original_hash,
			&wallet_address,
			chain_id,
		);

		let (pair, _) = sp_core::ecdsa::Pair::generate();
		let sig = pair.sign_prehashed(&replay_safe_hash);
		let account_id: AccountId32 =
			sp_io::hashing::blake2_256(pair.public().as_ref()).into();

		// Verify with wrong chain_id should fail
		let multi_sig = MultiSignature::K256WithPrefixCBS(
			sig,
			prefix,
			wallet_address,
			wrong_chain_id,
		);
		assert!(!multi_sig.verify(&tx_payload[..], &account_id));
	}

	#[test]
	fn cbs_wrong_wallet_address_fails() {
		let wallet_address: WalletAddress = hex!("1234567890abcdef1234567890abcdef12345678");
		let wrong_address: WalletAddress = hex!("aabbccddee1234567890abcdef1234567890abcd");
		let chain_id: u64 = 8453;
		let prefix: MessagePrefix = b"\x19Ethereum Signed Message:\n32".to_vec().try_into().unwrap();
		let tx_payload = b"test transaction payload";

		let original_hash = sp_io::hashing::keccak_256(
			&[prefix.as_slice(), ACURAST_SIGNATURE_PREFIX, tx_payload.as_slice()].concat(),
		);
		let replay_safe_hash = MultiSignature::compute_cbs_replay_safe_hash(
			&original_hash,
			&wallet_address,
			chain_id,
		);

		let (pair, _) = sp_core::ecdsa::Pair::generate();
		let sig = pair.sign_prehashed(&replay_safe_hash);
		let account_id: AccountId32 =
			sp_io::hashing::blake2_256(pair.public().as_ref()).into();

		// Verify with wrong wallet_address should fail
		let multi_sig = MultiSignature::K256WithPrefixCBS(
			sig,
			prefix,
			wrong_address,
			chain_id,
		);
		assert!(!multi_sig.verify(&tx_payload[..], &account_id));
	}

	#[test]
	fn cbs_tampered_message_fails() {
		let wallet_address: WalletAddress = hex!("1234567890abcdef1234567890abcdef12345678");
		let chain_id: u64 = 8453;
		let prefix: MessagePrefix = b"\x19Ethereum Signed Message:\n32".to_vec().try_into().unwrap();
		let tx_payload = b"test transaction payload";
		let tampered_payload = b"tampered transaction payload";

		let original_hash = sp_io::hashing::keccak_256(
			&[prefix.as_slice(), ACURAST_SIGNATURE_PREFIX, tx_payload.as_slice()].concat(),
		);
		let replay_safe_hash = MultiSignature::compute_cbs_replay_safe_hash(
			&original_hash,
			&wallet_address,
			chain_id,
		);

		let (pair, _) = sp_core::ecdsa::Pair::generate();
		let sig = pair.sign_prehashed(&replay_safe_hash);
		let account_id: AccountId32 =
			sp_io::hashing::blake2_256(pair.public().as_ref()).into();

		// Verify with tampered message should fail
		let multi_sig = MultiSignature::K256WithPrefixCBS(
			sig,
			prefix,
			wallet_address,
			chain_id,
		);
		assert!(!multi_sig.verify(&tampered_payload[..], &account_id));
	}

	#[test]
	fn cbs_scale_codec_roundtrip() {
		let wallet_address: WalletAddress = hex!("1234567890abcdef1234567890abcdef12345678");
		let chain_id: u64 = 8453;
		let prefix: MessagePrefix = b"\x19Ethereum Signed Message:\n32".to_vec().try_into().unwrap();
		let sig = ecdsa::Signature::from_raw([0u8; 65]);

		let multi_sig = MultiSignature::K256WithPrefixCBS(
			sig,
			prefix,
			wallet_address,
			chain_id,
		);

		let encoded = multi_sig.encode();
		// Variant index should be 8 (0-indexed: Ed25519=0, Sr25519=1, Ecdsa=2, P256=3,
		// P256WithAuthData=4, Ed25519WithPrefix=5, K256WithPrefix=6, Ed25519WithBase64=7,
		// K256WithPrefixCBS=8)
		assert_eq!(encoded[0], 8);

		let decoded = MultiSignature::decode(&mut &encoded[..]).unwrap();
		assert_eq!(multi_sig, decoded);
	}
}
