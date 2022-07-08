// use sp_runtime::{app_crypto::app_crypto, traits::Verify, MultiSignature, MultiSigner};

pub mod p256 {
	use codec::{Decode, Encode, MaxEncodedLen};
	use p256::ecdsa::{SigningKey, VerifyingKey};
	use p256::{PublicKey, SecretKey};
	use scale_info::TypeInfo;
	use sp_core::crypto::Infallible;
	use sp_runtime::app_crypto::{CryptoTypePublicPair, RuntimePublic, UncheckedFrom};
	use sp_runtime::{CryptoTypeId, KeyTypeId};
	use sp_runtime_interface::pass_by::PassByInner;
	use sp_std::vec::Vec;

	mod app {

		sp_runtime::app_crypto::app_crypto!(super, crate::KEY_TYPE);

		impl sp_runtime::BoundToRuntimeAppPublic for Public {
			type Public = Self;
		}
	}

	pub use app::Pair as AppPair;
	pub use app::{Public as AppPublic, Signature as AppSignature};

	#[derive(
		PartialEq,
		Eq,
		PartialOrd,
		Ord,
		Clone,
		Copy,
		Encode,
		Decode,
		PassByInner,
		MaxEncodedLen,
		TypeInfo,
		Hash,
		Debug,
	)]
	pub struct Public(pub [u8; 32]);

	impl Public {
		pub fn from_raw(data: [u8; 32]) -> Self {
			Public(data)
		}

		pub fn as_array_ref(&self) -> &[u8; 32] {
			self.as_ref()
		}
	}

	impl sp_core::Public for Public {
		fn to_public_crypto_pair(&self) -> CryptoTypePublicPair {
			CryptoTypePublicPair(CRYPTO_ID, self.to_raw_vec())
		}
	}

	impl sp_core::crypto::CryptoType for Public {
		type Pair = Pair;
	}

	impl sp_core::crypto::Derive for Public {
		fn derive<Iter: Iterator<Item = sp_core::DeriveJunction>>(
			&self,
			_path: Iter,
		) -> Option<Self> {
			todo!()
		}
	}

	impl sp_core::crypto::ByteArray for Public {
		const LEN: usize = 32;
	}

	impl AsRef<[u8; 32]> for Public {
		fn as_ref(&self) -> &[u8; 32] {
			&self.0
		}
	}

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

	impl std::ops::Deref for Public {
		type Target = [u8];

		fn deref(&self) -> &Self::Target {
			&self.0
		}
	}

	impl TryFrom<&[u8]> for Public {
		type Error = ();

		fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
			if data.len() != <Self as sp_core::crypto::ByteArray>::LEN {
				return Err(());
			}
			let mut r = [0u8; 32];
			r.copy_from_slice(data);
			Ok(Self::unchecked_from(r))
		}
	}

	impl UncheckedFrom<[u8; 32]> for Public {
		fn unchecked_from(x: [u8; 32]) -> Self {
			Public::from_raw(x)
		}
	}

	impl From<Public> for [u8; 32] {
		fn from(x: Public) -> [u8; 32] {
			x.0
		}
	}

	pub const CRYPTO_ID: CryptoTypeId = CryptoTypeId(*b"p256");

	#[derive(Encode, Decode, MaxEncodedLen, PassByInner, TypeInfo, PartialEq, Eq, Hash, Debug)]
	pub struct Signature(pub [u8; 64]);

	impl Clone for Signature {
		fn clone(&self) -> Self {
			let mut r = [0u8; 64];
			r.copy_from_slice(&self.0[..]);
			Signature(r)
		}
	}

	impl TryFrom<&[u8]> for Signature {
		type Error = ();

		fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
			if data.len() == 64 {
				let mut inner = [0u8; 64];
				inner.copy_from_slice(data);
				Ok(Signature(inner))
			} else {
				Err(())
			}
		}
	}

	impl AsRef<[u8; 64]> for Signature {
		fn as_ref(&self) -> &[u8; 64] {
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

	#[derive(Debug, Clone)]
	pub struct Pair {
		public: PublicKey,
		secret: SecretKey,
	}

	type Seed = [u8; 32];

	impl sp_core::Pair for Pair {
		type Public = Public;

		type Seed = Seed;

		type Signature = Signature;

		type DeriveError = Infallible;

		fn generate_with_phrase(password: Option<&str>) -> (Self, String, Self::Seed) {
			todo!()
		}

		fn from_phrase(
			phrase: &str,
			password: Option<&str>,
		) -> Result<(Self, Self::Seed), sp_runtime::app_crypto::SecretStringError> {
			todo!()
		}

		fn derive<Iter: Iterator<Item = sp_core::DeriveJunction>>(
			&self,
			path: Iter,
			seed: Option<Self::Seed>,
		) -> Result<(Self, Option<Self::Seed>), Self::DeriveError> {
			todo!()
		}

		fn from_seed(seed: &Self::Seed) -> Self {
			todo!()
		}

		fn from_seed_slice(seed: &[u8]) -> Result<Self, sp_runtime::app_crypto::SecretStringError> {
			todo!()
		}

		fn sign(&self, message: &[u8]) -> Self::Signature {
			use p256::ecdsa::signature::Signer;
			let key = SigningKey::from(&self.secret);
			let signature = key.sign(message).to_vec();
			(&signature[..]).try_into().unwrap()
		}

		fn verify<M: AsRef<[u8]>>(
			sig: &Self::Signature,
			message: M,
			pubkey: &Self::Public,
		) -> bool {
			use p256::ecdsa::signature::{Signature, Verifier};
			let key = VerifyingKey::from_sec1_bytes(&pubkey.0).unwrap();
			let signature = Signature::from_bytes(&sig.0).unwrap();
			key.verify(message.as_ref(), &signature).is_ok()
		}

		fn verify_weak<P: AsRef<[u8]>, M: AsRef<[u8]>>(sig: &[u8], message: M, pubkey: P) -> bool {
			use p256::ecdsa::signature::{Signature, Verifier};
			let key = VerifyingKey::from_sec1_bytes(pubkey.as_ref()).unwrap();
			let signature = Signature::from_bytes(sig).unwrap();
			key.verify(message.as_ref(), &signature).is_ok()
		}

		fn public(&self) -> Self::Public {
			todo!()
		}

		fn to_raw_vec(&self) -> Vec<u8> {
			todo!()
		}
	}

	impl sp_core::crypto::CryptoType for Pair {
		type Pair = Self;
	}

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
			sp_core::crypto::ByteArray::to_raw_vec(self)
		}
	}
}
