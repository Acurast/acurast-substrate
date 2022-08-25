pub mod p256 {
	#[cfg(feature = "std")]
	use bip39::{Language, Mnemonic, MnemonicType};
	use codec::{Decode, Encode, MaxEncodedLen};

	use p256::{
		ecdsa::{recoverable, signature::Signer, SigningKey, VerifyingKey},
		EncodedPoint, PublicKey, SecretKey,
	};
	use scale_info::TypeInfo;
	#[cfg(feature = "std")]
	use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
	use sp_runtime::traits::{IdentifyAccount, Lazy, Verify};
	use sp_runtime_interface::pass_by::PassByInner;

	#[cfg(feature = "std")]
	use sp_core::crypto::Ss58Codec;
	use sp_core::crypto::{
		ByteArray, CryptoType, CryptoTypeId, CryptoTypePublicPair, Derive, Public as TraitPublic,
		UncheckedFrom,
	};
	#[cfg(feature = "full_crypto")]
	use sp_core::{
		crypto::{DeriveJunction, Pair as TraitPair, SecretStringError},
		hashing::blake2_256,
	};

	/// An identifier used to match public keys against ecdsa keys
	pub const CRYPTO_ID: CryptoTypeId = CryptoTypeId(*b"p256");

	/// The ECDSA compressed public key.
	#[cfg_attr(feature = "full_crypto", derive(Hash))]
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

	impl TraitPublic for Public {
		fn to_public_crypto_pair(&self) -> CryptoTypePublicPair {
			CryptoTypePublicPair(CRYPTO_ID, self.to_raw_vec())
		}
	}

	impl From<Public> for CryptoTypePublicPair {
		fn from(key: Public) -> Self {
			(&key).into()
		}
	}

	impl From<&Public> for CryptoTypePublicPair {
		fn from(key: &Public) -> Self {
			CryptoTypePublicPair(CRYPTO_ID, key.to_raw_vec())
		}
	}

	impl ByteArray for Public {
		const LEN: usize = 33;
	}

	impl CryptoType for Public {
		#[cfg(feature = "full_crypto")]
		type Pair = Pair;
	}

	impl UncheckedFrom<[u8; 33]> for Public {
		fn unchecked_from(x: [u8; 33]) -> Self {
			Public(x)
		}
	}

	impl From<Public> for [u8; 33] {
		fn from(x: Public) -> [u8; 33] {
			x.0
		}
	}

	#[cfg(feature = "std")]
	impl std::fmt::Display for Public {
		fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
			write!(f, "{}", self.to_ss58check())
		}
	}

	impl sp_std::fmt::Debug for Public {
		#[cfg(feature = "std")]
		fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
			let s = self.to_ss58check();
			write!(f, "{} ({}...)", sp_core::hexdisplay::HexDisplay::from(&self.as_ref()), &s[0..8])
		}

		#[cfg(not(feature = "std"))]
		fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
			Ok(())
		}
	}

	#[cfg(feature = "std")]
	impl Serialize for Public {
		fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
		where
			S: Serializer,
		{
			serializer.serialize_str(&self.to_ss58check())
		}
	}

	#[cfg(feature = "std")]
	impl<'de> Deserialize<'de> for Public {
		fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
		where
			D: Deserializer<'de>,
		{
			Public::from_ss58check(&String::deserialize(deserializer)?)
				.map_err(|e| de::Error::custom(format!("{:?}", e)))
		}
	}

	impl IdentifyAccount for Public {
		type AccountId = Self;
		fn into_account(self) -> Self {
			self
		}
	}

	#[cfg(feature = "full_crypto")]
	type Seed = [u8; 32];

	/// A signature (a 512-bit value, plus 8 bits for recovery ID).
	#[cfg_attr(any(feature = "std", feature = "full_crypto"), derive(Hash))]
	#[derive(Encode, Decode, MaxEncodedLen, PassByInner, TypeInfo, PartialEq, Eq)]
	pub struct Signature(pub [u8; 65]);

	impl TryFrom<recoverable::Signature> for Signature {
		type Error = ();

		fn try_from(data: recoverable::Signature) -> Result<Self, Self::Error> {
			let signature_bytes = p256::ecdsa::signature::Signature::as_bytes(&data);
			Signature::try_from(signature_bytes)
		}
	}

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

	#[cfg(feature = "std")]
	impl Serialize for Signature {
		fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
		where
			S: Serializer,
		{
			serializer.serialize_str(&hex::encode(self))
		}
	}

	#[cfg(feature = "std")]
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

	impl CryptoType for Signature {
		#[cfg(feature = "full_crypto")]
		type Pair = Pair;
	}

	impl Verify for Signature {
		type Signer = Public;

		fn verify<L: Lazy<[u8]>>(&self, mut msg: L, signer: &Self::Signer) -> bool {
			match PublicKey::from_sec1_bytes(signer.as_ref()) {
				Ok(public_key) => {
					let message = msg.get();
					let signature_bytes: &[u8] = self.as_ref();
					let verifying_key = VerifyingKey::from(public_key);
					let verifying_key_from_signature =
						recoverable::Signature::try_from(signature_bytes.as_ref())
							.unwrap()
							.recover_verify_key(message)
							.unwrap();

					verifying_key == verifying_key_from_signature
				},
				Err(_) => false,
			}
		}
	}

	/// A key pair.
	#[derive(Clone)]
	pub struct Pair {
		public: Public,
		#[cfg(feature = "full_crypto")]
		secret: SecretKey,
	}

	impl Pair {
		pub fn generate_from_seed_bytes(bytes: &[u8]) -> Result<Self, ()> {
			let secret = SecretKey::from_be_bytes(bytes).map_err(|_error| ())?;
			let public = secret.public_key();
			let pub_bytes = public.to_bytes();
			Ok(Pair {
				public: Public(pub_bytes),
				#[cfg(feature = "full_crypto")]
				secret,
			})
		}

		pub fn get_public(&self) -> Public {
			self.public
		}
	}

	#[cfg(feature = "full_crypto")]
	impl CryptoType for Pair {
		type Pair = Pair;
	}

	/// An error when deriving a key.
	#[cfg(feature = "full_crypto")]
	pub enum DeriveError {
		/// A soft key was found in the path (and is unsupported).
		SoftKeyInPath,
	}

	/// Derive a single hard junction.
	#[cfg(feature = "full_crypto")]
	fn derive_hard_junction(secret_seed: &Seed, cc: &[u8; 32]) -> Seed {
		("Secp256r1", secret_seed, cc).using_encoded(blake2_256)
	}

	trait ToBytes33 {
		fn to_bytes(&self) -> [u8; 33];
	}

	impl ToBytes33 for p256::PublicKey {
		fn to_bytes(&self) -> [u8; 33] {
			let encoded_point = EncodedPoint::from(self);
			let compressed_point = encoded_point.compress();
			compressed_point.as_bytes().try_into().unwrap()
		}
	}

	#[cfg(feature = "full_crypto")]
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
			Self::from_seed_slice(&seed[..]).expect("seed has valid length; qed")
		}

		/// Make a new key pair from secret seed material. The slice must be 32 bytes long or it
		/// will return `None`.
		///
		/// You should never need to use this; generate(), generate_with_phrase
		fn from_seed_slice(seed_slice: &[u8]) -> Result<Pair, SecretStringError> {
			Self::generate_from_seed_bytes(seed_slice).map_err(|_| SecretStringError::InvalidSeed)
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
			let key = SigningKey::from(&self.secret);
			let p256_signature: recoverable::Signature = key.sign(message);

			Signature::try_from(p256_signature).expect("invalid signature")
		}

		/// Verify a signature on a message. Returns true if the signature is good.
		fn verify<M: AsRef<[u8]>>(
			sig: &Self::Signature,
			message: M,
			pubkey: &Self::Public,
		) -> bool {
			sig.verify(message.as_ref(), pubkey)
		}

		/// Verify a signature on a message. Returns true if the signature is good.
		///
		/// This doesn't use the type system to ensure that `sig` and `pubkey` are the correct
		/// size. Use it only if you're coming from byte buffers and need the speed.
		fn verify_weak<P: AsRef<[u8]>, M: AsRef<[u8]>>(sig: &[u8], message: M, pubkey: P) -> bool {
			// TODO: weak version, for now use normal verify
			let signature = match Self::Signature::try_from(sig) {
				Err(_) => return false,
				Ok(sign) => sign,
			};
			let public = match Self::Public::try_from(pubkey.as_ref()) {
				Err(_) => return false,
				Ok(pk) => pk,
			};

			Self::verify(&signature, message, &public)
		}

		/// Return a vec filled with raw data.
		fn to_raw_vec(&self) -> Vec<u8> {
			self.seed().to_vec()
		}
	}

	#[cfg(feature = "full_crypto")]
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
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::application_crypto::p256::Pair;
	use hex_literal::hex;
	use sp_application_crypto::DeriveJunction;
	use sp_core::{
		crypto::{Pair as TraitPair, DEV_PHRASE},
		hashing::blake2_256,
	};
	use sp_runtime::AccountId32;

	fn build_dummy_pair() -> Pair {
		let seed = "Test";
		Pair::from_string(&format!("//{}", seed), None).expect("static values are valid; qed")
	}

	#[test]
	fn generate_account_id() {
		let pair = build_dummy_pair();

		let account_id: AccountId32 = blake2_256(pair.get_public().as_ref()).into();
		assert_eq!("5CahxeGW24hPXsUTZsiiBgsuBbsQqga8oY6ai4uKMm5X4wym", account_id.to_string());
	}

	#[test]
	fn test_account() {
		let pair = build_dummy_pair();

		let payload = hex!("0a000090b5ab205c6974c9ea841be688864633dc9ca8a357843eeacf2314649965fe22070010a5d4e84502000001000000010000003ce9390c8bd3361b348592b2c3008ece6c530e415821abb9759215e8dc83f0490e70b9cbbbcd07a80821fd7dfca9c93ae922688b37a484d5fd68dedcc2cabaa5");

		let signature = pair.sign(&payload);
		assert!(Pair::verify(&signature, &payload, &pair.public()));
	}

	#[test]
	fn default_phrase_should_be_used() {
		assert_eq!(
			Pair::from_string("//Alice///password", None).unwrap().public(),
			Pair::from_string(&format!("{}//Alice", DEV_PHRASE), Some("password"))
				.unwrap()
				.public(),
		);
	}

	#[test]
	fn seed_and_derive_should_work() {
		let seed = hex!("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
		let pair = Pair::from_seed(&seed);
		assert_eq!(pair.seed(), seed);
		let path = vec![DeriveJunction::Hard([0u8; 32])];
		let derived = pair.derive(path.into_iter(), None).ok().unwrap();
		assert_eq!(
			derived.0.seed(),
			hex!("6188237fc80465cd043c58ac7623eaefa9f4db5ce8dee2cd00c6458c5303cf30")
		);
	}

	#[test]
	fn test_vector_should_work() {
		let seed = hex!("f67b03b2c6e4bf86cce50298dbce351b332c3be65ced9f312b6d9ffc3de6b04f");
		let pair = Pair::from_seed(&seed);
		let public = pair.public();

		let public_key_bytes =
			hex!("02c156afee1ce52ef83a0dd168c1144eb20008697e6664fa132ba23c128cce8055");
		assert_eq!(public, p256::Public::from_raw(public_key_bytes),);
		let message = b"".to_vec();

		let signature = hex!("696e710fc4516d0a2ba91162777b5f0a4d0e9849a6121a4bae00a0d2df70b5d2ef6e26b0191024872aa22530ed3bef47cd8b0c635e659c79a4cc4a1533013b9c01");
		let signature = p256::Signature(signature);

		assert!(pair.sign(&message[..]) == signature);
		assert!(Pair::verify(&signature, &message[..], &public));
	}

	#[test]
	fn test_vector_by_string_should_work() {
		let pair = Pair::from_string(
			"0xf67b03b2c6e4bf86cce50298dbce351b332c3be65ced9f312b6d9ffc3de6b04f",
			None,
		)
		.unwrap();
		let public = pair.public();
		assert_eq!(
			public,
			p256::Public::from_raw(hex!(
				"02c156afee1ce52ef83a0dd168c1144eb20008697e6664fa132ba23c128cce8055"
			)),
		);
		let message = b"";
		let signature = hex!("696e710fc4516d0a2ba91162777b5f0a4d0e9849a6121a4bae00a0d2df70b5d2ef6e26b0191024872aa22530ed3bef47cd8b0c635e659c79a4cc4a1533013b9c01");
		let signature = p256::Signature(signature);
		assert!(pair.sign(&message[..]) == signature);
		assert!(Pair::verify(&signature, &message[..], &public));
	}
}
