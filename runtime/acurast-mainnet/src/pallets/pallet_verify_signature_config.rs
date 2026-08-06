use crate::Runtime;
use acurast_runtime_common::types::Signature;

/// Runtime configuration for pallet_verify_signature.
///
/// This pallet exists only for its `VerifySignature` transaction extension, which is what allows a
/// v5 "general" transaction to establish its origin: such transactions carry no signature in the
/// preamble, so an extension has to verify one and authorize the origin.
///
/// It is what makes transaction extension version 1 usable. `Preamble::Signed` (extrinsic v4) is
/// hardcoded to extension version 0 in the SDK, so version 1 — the pipeline carrying
/// `CheckMetadataHash` for the Ledger Polkadot Generic app — is only reachable through a general
/// transaction. The pallet declares no calls, so it adds nothing to the call surface.
impl pallet_verify_signature::Config for Runtime {
	/// The chain's own signature type, which already supports P-256 alongside the standard schemes.
	type Signature = Signature;
	/// `MultiSigner` resolves a verified signer to an `AccountId32`, matching this runtime's
	/// `AccountId`, so no adapter is needed.
	type AccountIdentifier = acurast_p256_crypto::MultiSigner;
	type WeightInfo = pallet_verify_signature::weights::SubstrateWeight<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarking::SignatureHelper;
}

/// The blanket `BenchmarkHelper` impl in the pallet covers `sp_runtime::MultiSignature`, not this
/// chain's `acurast_p256_crypto::MultiSignature`, so the sr25519 variant is produced here.
#[cfg(feature = "runtime-benchmarks")]
mod benchmarking {
	use super::*;
	use sp_core::crypto::AccountId32;
	use sp_io::crypto::{sr25519_generate, sr25519_sign};
	use sp_runtime::traits::IdentifyAccount;

	pub struct SignatureHelper;

	impl pallet_verify_signature::BenchmarkHelper<Signature, AccountId32> for SignatureHelper {
		fn create_signature(_entropy: &[u8], msg: &[u8]) -> (Signature, AccountId32) {
			let public = sr25519_generate(0.into(), None);
			let account = acurast_p256_crypto::MultiSigner::Sr25519(public).into_account();
			let signature = Signature::Sr25519(
				sr25519_sign(0.into(), &public, msg).expect("benchmark signing key exists; qed"),
			);
			(signature, account)
		}
	}
}
