//! ECDSA secp256r1 API.
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::ecdsa::DeriveError;
use sp_runtime_interface::pass_by::PassByInner;
use sp_core::crypto::Ss58Codec;
use sp_runtime::app_crypto::{CryptoTypePublicPair, RuntimePublic, UncheckedFrom};
use sp_runtime::{CryptoTypeId, KeyTypeId};
use sp_core::crypto::{
	ByteArray, CryptoType, Derive, Public as TraitPublic,
};

use sp_core::{
	crypto::{DeriveJunction, Pair as TraitPair, SecretStringError},
	hashing::blake2_256,
};

use p256::{
	PublicKey, SecretKey, EncodedPoint, ecdsa::{SigningKey}
};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use sp_std::vec::Vec;

/// An identifier used to match public keys against ecdsa keys
pub const CRYPTO_ID: CryptoTypeId = CryptoTypeId(*b"p256");

/// A secret seed (which is bytewise essentially equivalent to a SecretKey).
///
/// We need it as a different type because `Seed` is expected to be AsRef<[u8]>.
type Seed = [u8; 32];

use bip39::{Language, Mnemonic, MnemonicType};

/// The ECDSA compressed public key.
#[derive(
	Clone,
	Copy,
	Encode,
	Decode,
	PassByInner,
	MaxEncodedLen,
	TypeInfo,
	Eq,
	PartialEq,
	PartialOrd,
	Ord,
	Hash
)]
pub struct Public(pub [u8; 33]);

impl Public {
	/// A new instance from the given 33-byte `data`.
	///
	/// NOTE: No checking goes on to ensure this is a real public key. Only use it if
	/// you are certain that the array actually is a pubkey. GIGO!
	pub fn from_raw(data: [u8; 33]) -> Self {
		Self(data)
	}

	/// Create a new instance from the given full public key.
	///
	/// This will convert the full public key into the compressed format.
	pub fn from_full(full: &[u8]) -> Result<Self, p256::elliptic_curve::Error> {
        let pubkey_conversion = if full.len() == 64 {
            // Tag it as uncompressed public key.
            let mut tagged_full = [0u8; 65];
            tagged_full[0] = 0x04;
            tagged_full[1..].copy_from_slice(full);
            PublicKey::from_sec1_bytes(&tagged_full)
        } else {
            PublicKey::from_sec1_bytes(full)
        };
        // return Err if conversion from bytes is unsuccessful

        match pubkey_conversion {
            Err(e) => Err(e),

            Ok(pubkey) => {
                let encoded_point = EncodedPoint::from(pubkey);
                let compressed_point = encoded_point.compress();
                let compressed_array = compressed_point.as_bytes().try_into().unwrap();
                
                Ok(Public(compressed_array))	// return Ok if successfull
            }
        }
	}
}

impl ByteArray for Public {
	const LEN: usize = 33;
}

impl CryptoType for Public {
	type Pair = Pair;
}

impl TraitPublic for Public {
	fn to_public_crypto_pair(&self) -> CryptoTypePublicPair {
		CryptoTypePublicPair(CRYPTO_ID, RuntimePublic::to_raw_vec(self))
	}
}

impl From<Public> for CryptoTypePublicPair {
	fn from(key: Public) -> Self {
		(&key).into()
	}
}

impl From<&Public> for CryptoTypePublicPair {
	fn from(key: &Public) -> Self {
		CryptoTypePublicPair(CRYPTO_ID, RuntimePublic::to_raw_vec(key))
	}
}

impl Derive for Public {}

impl AsRef<[u8]> for Public {
	fn as_ref(&self) -> &[u8] {
		&self.0[..]
	}
}

impl AsMut<[u8]> for Public {
	fn as_mut(&mut self) -> &mut [u8] {
		&mut self.0[..]
	}
}

impl TryFrom<&[u8]> for Public {
	type Error = ();

	fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
		if data.len() != Self::LEN {
			return Err(())
		}
		let mut r = [0u8; Self::LEN];
		r.copy_from_slice(data);
		Ok(Self::unchecked_from(r))
	}
}

impl From<Pair> for Public {
	fn from(x: Pair) -> Self {
		x.public()
	}
}

impl UncheckedFrom<[u8; 33]> for Public {
	fn unchecked_from(x: [u8; 33]) -> Self {
		Public(x)
	}
}

impl std::fmt::Display for Public {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "{}", self.to_ss58check())
	}
}

impl sp_std::fmt::Debug for Public {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		let s = self.to_ss58check();
		write!(f, "{} ({}...)", sp_core::hexdisplay::HexDisplay::from(&self.as_ref()), &s[0..8])
	}

	// fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
	// 	Ok(())
	// }
}

impl Serialize for Public {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		serializer.serialize_str(&self.to_ss58check())
	}
}

impl<'de> Deserialize<'de> for Public {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		Public::from_ss58check(&String::deserialize(deserializer)?)
			.map_err(|e| de::Error::custom(format!("{:?}", e)))
	}
}

/// A signature (a 512-bit value, plus 8 bits for recovery ID).
#[derive(Encode, Decode, MaxEncodedLen, PassByInner, TypeInfo, PartialEq, Eq, Hash)]
pub struct Signature(pub [u8; 65]);

impl TryFrom<&[u8]> for Signature {
	type Error = ();

	fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
		if data.len() == 65 {
			let mut inner = [0u8; 65];
			inner.copy_from_slice(data);
			Ok(Signature(inner))
		} else {
			Err(())
		}
	}
}

impl Serialize for Signature {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		serializer.serialize_str(&hex::encode(self))
	}
}

impl<'de> Deserialize<'de> for Signature {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		let signature_hex = hex::decode(&String::deserialize(deserializer)?)
			.map_err(|e| de::Error::custom(format!("{:?}", e)))?;
		Signature::try_from(signature_hex.as_ref())
			.map_err(|e| de::Error::custom(format!("{:?}", e)))
	}
}

impl Clone for Signature {
	fn clone(&self) -> Self {
		let mut r = [0u8; 65];
		r.copy_from_slice(&self.0[..]);
		Signature(r)
	}
}

impl Default for Signature {
	fn default() -> Self {
		Signature([0u8; 65])
	}
}

impl From<Signature> for [u8; 65] {
	fn from(v: Signature) -> [u8; 65] {
		v.0
	}
}

impl AsRef<[u8; 65]> for Signature {
	fn as_ref(&self) -> &[u8; 65] {
		&self.0
	}
}

impl AsRef<[u8]> for Signature {
	fn as_ref(&self) -> &[u8] {
		&self.0[..]
	}
}

impl AsMut<[u8]> for Signature {
	fn as_mut(&mut self) -> &mut [u8] {
		&mut self.0[..]
	}
}

impl sp_std::fmt::Debug for Signature {
	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		write!(f, "{}", sp_core::hexdisplay::HexDisplay::from(&self.0))
	}
}

impl UncheckedFrom<[u8; 65]> for Signature {
	fn unchecked_from(data: [u8; 65]) -> Signature {
		Signature(data)
	}
}


/// A key pair.
#[derive(Clone)]
pub struct Pair {
	public: Public,
	secret: SecretKey,
}

/// Derive a single hard junction.
fn derive_hard_junction(secret_seed: &Seed, cc: &[u8; 32]) -> Seed {
	("Secp256r1", secret_seed, cc).using_encoded(blake2_256)
}

impl CryptoType for Pair {
	type Pair = Pair;
}

impl TraitPair for Pair {
	type Public = Public;
	type Seed = Seed;
	type Signature = Signature;
	type DeriveError = DeriveError;
	
	// Using default fn generate() 

	/// Generate new secure (random) key pair and provide the recovery phrase.
	///
	/// You can recover the same key later with `from_phrase`.
	fn generate_with_phrase(password: Option<&str>) -> (Pair, String, Seed) {
		let mnemonic = Mnemonic::new(MnemonicType::Words12, Language::English);
		let phrase = mnemonic.phrase();
		let (pair, seed) = Self::from_phrase(phrase, password)
			.expect("All phrases generated by Mnemonic are valid; qed");
		(pair, phrase.to_owned(), seed)
	}

	/// Generate key pair from given recovery phrase and password.
	fn from_phrase(
		phrase: &str,
		password: Option<&str>,
	) -> Result<(Pair, Seed), SecretStringError> {
		let big_seed = substrate_bip39::seed_from_entropy(
			Mnemonic::from_phrase(phrase, Language::English)
				.map_err(|_| SecretStringError::InvalidPhrase)?
				.entropy(),
			password.unwrap_or(""),
		)
		.map_err(|_| SecretStringError::InvalidSeed)?;
		let mut seed = Seed::default();
		seed.copy_from_slice(&big_seed[0..32]);
		Self::from_seed_slice(&big_seed[0..32]).map(|x| (x, seed))
	}

	/// Make a new key pair from secret seed material.
	///
	/// You should never need to use this; generate(), generate_with_phrase
	fn from_seed(seed: &Seed) -> Pair {
		return Self::from_seed_slice(&seed[..]).expect("seed has valid length; qed")
	}
	
	/// Make a new key pair from secret seed material. The slice must be 32 bytes long or it
	/// will return `None`.
	///
	/// You should never need to use this; generate(), generate_with_phrase
	fn from_seed_slice(seed_slice: &[u8]) -> Result<Pair, SecretStringError> {
		let secret = SecretKey::from_be_bytes(seed_slice).expect("NistP256 Secretkey converison not possible");
		let public = secret.public_key();
		let pub_bytes = {
			let encoded_point = EncodedPoint::from(public);
			let compressed_point = encoded_point.compress();
			compressed_point.as_bytes().try_into().unwrap()
		};
		Ok(Pair {public: Public(pub_bytes), secret})
	}

	/// Derive a child key from a series of given junctions.
	fn derive<Iter: Iterator<Item = DeriveJunction>>(
		&self,
		path: Iter,
		_seed: Option<Seed>,
	) -> Result<(Pair, Option<Seed>), DeriveError> {
		let mut acc = self.seed();
		for j in path {
			match j {
				DeriveJunction::Soft(_cc) => return Err(DeriveError::SoftKeyInPath),
				DeriveJunction::Hard(cc) => acc = derive_hard_junction(&acc, &cc),
			}
		}
		Ok((Self::from_seed(&acc), Some(acc)))
	}

	/// Get the public key.
	fn public(&self) -> Public {
		self.public
	}

	/// Sign a message.
	fn sign(&self, message: &[u8]) -> Signature {
		use p256::ecdsa::signature::Signer;
		let key = SigningKey::from(&self.secret);
		let p256_signature = key.sign(message);
		// Signature(*p256_signinature.as_ref().try_into().expect("couldn't convert into 64 byte array"))
		let sig_vec = p256_signature.to_vec();
		let signature: Self::Signature = (&sig_vec[..]).try_into().unwrap();
		return signature
	}

	/// Verify a signature on a message. Returns true if the signature is good.
	fn verify<M: AsRef<[u8]>>(sig: &Self::Signature, message: M, pubkey: &Self::Public) -> bool {
		use p256::ecdsa::VerifyingKey;
		let public_key = match PublicKey::from_sec1_bytes(pubkey.as_ref()) {
			Ok(pk) => pk,
			Err(_) => return false
		};

		let verifying_key = VerifyingKey::from(public_key);

		let signature = match p256::ecdsa::Signature::try_from(sig.as_ref()) {
			Ok(sign) => sign,
			Err(_) => return false
		};

		match p256::ecdsa::signature::Verifier::verify(&verifying_key, message.as_ref(), &signature) {
			Ok(_) => return true,
			_ => return false
		}
	}
	
	/// Verify a signature on a message. Returns true if the signature is good.
	///
	/// This doesn't use the type system to ensure that `sig` and `pubkey` are the correct
	/// size. Use it only if you're coming from byte buffers and need the speed.
	fn verify_weak<P: AsRef<[u8]>, M: AsRef<[u8]>>(sig: &[u8], message: M, pubkey: P) -> bool {
		// TODO: weak version, for now use normal verify
		let signature = match Self::Signature::try_from(sig){
			Err(_) => return false,
			Ok(sign) => sign
		};
		let public = match Self::Public::try_from(pubkey.as_ref()){
			Err(_) => return false,
			Ok(pk) => pk
		};
		
		Self::verify(&signature, message, &public)
	}

	/// Return a vec filled with raw data.
	fn to_raw_vec(&self) -> Vec<u8> {
		self.seed().to_vec()
	}
}

impl Pair {
	/// Get the seed for this key.
	pub fn seed(&self) -> Seed {
		*self.secret.to_be_bytes().as_ref()
	}

	/// Exactly as `from_string` except that if no matches are found then, the the first 32
	/// characters are taken (padded with spaces as necessary) and used as the MiniSecretKey.
	pub fn from_legacy_string(s: &str, password_override: Option<&str>) -> Pair {
		Self::from_string(s, password_override).unwrap_or_else(|_| {
			let mut padded_seed: Seed = [b' '; 32];
			let len = s.len().min(32);
			padded_seed[..len].copy_from_slice(&s.as_bytes()[..len]);
			Self::from_seed(&padded_seed)
		})
	}

}

// we require access to keystore because the implemented methods don't work on new algorithms.
// TODO: find access to keystore
impl RuntimePublic for Public {
	type Signature = Signature;

	fn all(key_type: KeyTypeId) -> Vec<Self> {
		// sp_io::crypto::sr25519_public_keys(key_type)
		todo!()
	}

	fn generate_pair(key_type: KeyTypeId, seed: Option<Vec<u8>>) -> Self {
		// sp_io::crypto::sr25519_generate(key_type, seed)
		todo!()
	}

	fn sign<M: AsRef<[u8]>>(&self, key_type: KeyTypeId, msg: &M) -> Option<Self::Signature> {
		// sp_io::crypto::sr25519_sign(key_type, self, msg.as_ref())
		todo!()
	}

	fn verify<M: AsRef<[u8]>>(&self, msg: &M, signature: &Self::Signature) -> bool {
		// sp_io::crypto::sr25519_verify(signature, msg.as_ref(), self)
		todo!()
	}

	fn to_raw_vec(&self) -> Vec<u8> {
		// sp_core::crypto::ByteArray::to_raw_vec(self)
		todo!()
	}
}