//! `benchmark overhead` extrinsic builder signing with P-256, so `ExtrinsicBaseWeight` covers the
//! in-wasm P-256 verification most transactions pay rather than the sr25519 host call.
//! Mirrors `frame_benchmarking_cli`'s `DynamicRemarkBuilder`, which is hardwired to sr25519.

use acurast_p256_crypto::{core::p256::Pair, MultiSignature, MultiSigner};
use frame_benchmarking_cli::ExtrinsicBuilder;
use sp_core::Pair as _;
use sp_runtime::{traits::IdentifyAccount, OpaqueExtrinsic};
use subxt::{
	client::RuntimeVersion,
	config::{
		substrate::{
			AccountId32, BlakeTwo256, MultiAddress, SubstrateExtrinsicParamsBuilder,
			SubstrateHeader, H256,
		},
		Config, SubstrateExtrinsicParams,
	},
	tx::Signer,
	Metadata, OfflineClient,
};

/// Signer of the benchmarked remarks; `ci/benchmark.sh` endows its account.
const SIGNER_SURI: &str = "//Alice";

pub enum AcurastConfig {}

impl Config for AcurastConfig {
	type AccountId = AccountId32;
	type Address = MultiAddress<AccountId32, u32>;
	type Signature = MultiSignature;
	type Hasher = BlakeTwo256;
	type Header = SubstrateHeader<u32, BlakeTwo256>;
	type ExtrinsicParams = SubstrateExtrinsicParams<Self>;
	type AssetId = u32;
}

struct P256Signer(Pair);

impl Signer<AcurastConfig> for P256Signer {
	fn account_id(&self) -> AccountId32 {
		AccountId32(MultiSigner::P256(self.0.public()).into_account().into())
	}

	fn sign(&self, signer_payload: &[u8]) -> MultiSignature {
		MultiSignature::P256(self.0.sign(signer_payload))
	}
}

/// Builds `System::remark` extrinsics signed by `SIGNER_SURI` as P-256.
pub struct P256RemarkBuilder {
	client: OfflineClient<AcurastConfig>,
	signer: P256Signer,
}

impl P256RemarkBuilder {
	pub fn new(metadata: Metadata, genesis: H256, version: RuntimeVersion) -> Self {
		let pair = Pair::from_string(SIGNER_SURI, None).expect("static dev SURI is valid; qed");
		Self { client: OfflineClient::new(genesis, version, metadata), signer: P256Signer(pair) }
	}
}

impl ExtrinsicBuilder for P256RemarkBuilder {
	fn pallet(&self) -> &str {
		"system"
	}

	fn extrinsic(&self) -> &str {
		"remark"
	}

	fn build(&self, nonce: u32) -> Result<OpaqueExtrinsic, &'static str> {
		let call = subxt::dynamic::tx("System", "remark", vec![Vec::<u8>::new()]);
		let params = SubstrateExtrinsicParamsBuilder::<AcurastConfig>::new()
			.nonce(nonce.into())
			.build();
		let mut encoded = self
			.client
			.tx()
			.create_partial_offline(&call, params)
			.map_err(|_| "Unable to create remark")?
			.sign(&self.signer)
			.into_encoded();
		OpaqueExtrinsic::try_from_encoded_extrinsic(&mut encoded)
			.map_err(|_| "Unable to construct OpaqueExtrinsic")
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use sp_core::crypto::Ss58Codec;

	#[test]
	fn signer_matches_account_endowed_by_ci() {
		let pair = Pair::from_string(SIGNER_SURI, None).unwrap();
		assert_eq!(
			MultiSigner::P256(pair.public()).into_account().to_ss58check(),
			"5GcMbLqv9iziFaUzhi3EgNwAfSjbtUEdYBsDGBeoy8DctUxk",
		);
	}
}
