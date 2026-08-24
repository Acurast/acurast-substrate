use derive_more::{From, Into};
use frame_support::{
	traits::{Currency, EitherOfDiverse},
	weights::{WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial},
};
use frame_system::EnsureRoot;
use pallet_acurast_processor_manager::onboarding::Onboarding;
use smallvec::smallvec;
use sp_runtime::{generic, impl_opaque_keys, AccountId32, Perbill};
use sp_std::prelude::*;

use acurast_runtime_common::{
	check_nonce::CheckNonce,
	constants::{
		BLOCK_PROCESSING_VELOCITY, MILLIUNIT, RELAY_CHAIN_SLOT_DURATION_MILLIS,
		UNINCLUDED_SEGMENT_CAPACITY,
	},
	migrations::storage_versions::StorageVersionBackfill,
	opaque,
	types::{AccountId, Address, Balance, CouncilFourSeventh, Signature},
	weight::ExtrinsicBaseWeight,
};

use crate::{AcurastProcessorManager, AllPalletsWithSystem, Aura, Balances, Runtime, RuntimeCall};

/// Wrapper around [`AccountId32`] to allow the implementation of [`TryFrom<Vec<u8>>`].
#[derive(Debug, From, Into, Clone, Eq, PartialEq)]
pub struct AcurastAccountId(AccountId32);
impl TryFrom<Vec<u8>> for AcurastAccountId {
	type Error = ();

	fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
		let a: [u8; 32] = value.try_into().map_err(|_| ())?;
		Ok(AcurastAccountId(AccountId32::new(a)))
	}
}

/// Block type as expected by this runtime.
pub type Block = generic::Block<opaque::Header, UncheckedExtrinsic>;

/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;

/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;

/// The transaction extension pipeline at version 0.
///
/// This is byte-for-byte identical to the extension line used before the versioned-extrinsic
/// upgrade. It is the pipeline applied to legacy (v4) signed transactions and to v5 "general"
/// transactions that declare extension version 0, so existing clients keep working unchanged.
pub type TransactionExtensionV0 = cumulus_pallet_weight_reclaim::StorageWeightReclaim<
	Runtime,
	(
		frame_system::AuthorizeCall<Runtime>,
		frame_system::CheckNonZeroSender<Runtime>,
		frame_system::CheckSpecVersion<Runtime>,
		frame_system::CheckTxVersion<Runtime>,
		frame_system::CheckGenesis<Runtime>,
		frame_system::CheckEra<Runtime>,
		Onboarding<Runtime, AcurastProcessorManager>,
		CheckNonce<Runtime, AcurastProcessorManager>,
		frame_system::CheckWeight<Runtime>,
		pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
	),
>;

/// Backwards-compatible alias for the version 0 transaction extension pipeline.
pub type TxExtension = TransactionExtensionV0;

// ---------------------------------------------------------------------------------------------
// Versioned transaction extensions — DISABLED, kept for reference.
//
// The intent was to add `CheckMetadataHash` (RFC-0078, required by the Ledger Polkadot Generic app)
// in a *second* extension version, so the ~30k processors — which build their signed payload from a
// hardcoded extension list — could keep using version 0 untouched.
//
// It works on-chain: a v5 general transaction selecting extension version 1 was built, signed and
// applied against a local node, and unsigned attempts were correctly rejected with `UnknownOrigin`.
// The blocker is client tooling, and it breaks both libraries we depend on, in opposite ways:
//
//   * @polkadot/api 16.5.6 keeps a single registry-wide extension list. From metadata v16 it
//     flattens every version into that one list, so it encoded v1's `CheckMetadataHash` byte into
//     v4 transactions and every extrinsic failed to decode. It also cannot decode a block that
//     contains a v1 extrinsic. Per-version support is only an open RFC (polkadot-js/api#6213).
//   * subxt picks the *highest* advertised version, so it would select v1 and fail on the
//     extensions it does not know (paritytech/subxt#1998, #2265 — both open).
//
// Re-enabling this needs those upstream gaps closed. The alternative path, which needs none of
// this, is to teach the processor client to pick its layout from `spec_version` and then move
// `CheckMetadataHash` into the single pipeline in a coordinated runtime upgrade.
// ---------------------------------------------------------------------------------------------
// /// The transaction extension pipeline at version 1.
// ///
// /// Identical to [`TransactionExtensionV0`] with [`frame_metadata_hash_extension::CheckMetadataHash`]
// /// appended. The Ledger Polkadot Generic app requires this extension (RFC-0078) to verify and
// /// display transactions offline. Clients opt in by building a v5 general transaction with extension
// /// version 1; clients that do not know about it keep using version 0 and are unaffected.
// pub type TransactionExtensionV1 = cumulus_pallet_weight_reclaim::StorageWeightReclaim<
// 	Runtime,
// 	(
// 		// Must come first: it requires an as-yet unauthorized origin and rejects with
// 		// `BadSigner` otherwise, and it verifies the signature over the call plus the data of
// 		// every extension that FOLLOWS it. Anything placed before it is excluded from the
// 		// signed payload.
// 		pallet_verify_signature::VerifySignature<Runtime>,
// 		frame_system::AuthorizeCall<Runtime>,
// 		frame_system::CheckNonZeroSender<Runtime>,
// 		frame_system::CheckSpecVersion<Runtime>,
// 		frame_system::CheckTxVersion<Runtime>,
// 		frame_system::CheckGenesis<Runtime>,
// 		frame_system::CheckEra<Runtime>,
// 		Onboarding<Runtime, AcurastProcessorManager>,
// 		CheckNonce<Runtime, AcurastProcessorManager>,
// 		frame_system::CheckWeight<Runtime>,
// 		pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
// 		frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
// 	),
// >;
//
// /// The transaction extension versions supported by this runtime in addition to version 0.
// ///
// /// New versions can be appended here as `PipelineAtVers<N, ..>` entries without disturbing the
// /// encoding of existing versions.
// pub type OtherVersions =
// 	sp_runtime::traits::MultiVersion<sp_runtime::traits::PipelineAtVers<1, TransactionExtensionV1>>;

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
	generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, TransactionExtensionV0>;

/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic =
	generic::CheckedExtrinsic<AccountId, RuntimeCall, TransactionExtensionV0>;

/// Runtime migrations executed once on the next runtime upgrade.
pub type Migrations = (StorageVersionBackfill<Runtime>,);

/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Runtime,
	Block,
	frame_system::ChainContext<Runtime>,
	Runtime,
	AllPalletsWithSystem,
	Migrations,
>;

/// Handles converting a weight scalar to a fee value, based on the scale and granularity of the
/// node's balance type.
///
/// This should typically create a mapping between the following ranges:
///   - `[0, MAXIMUM_BLOCK_WEIGHT]`
///   - `[Balance::min, Balance::max]`
///
/// Yet, it can be used for any other sort of change to weight-fee. Some examples being:
///   - Setting it to `0` will essentially disable the weight fee.
///   - Setting it to `1` will cause the literal `#[weight = x]` values to be charged.
pub struct WeightToFee;
impl WeightToFeePolynomial for WeightToFee {
	type Balance = Balance;
	fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
		// in Kusama, extrinsic base weight (smallest non-zero weight) is mapped to 1 MILLIUNIT:
		// for acurast, we map to 1/10 of that, or 1/10 MILLIUNIT
		let p = MILLIUNIT / 10;
		let q = 100 * Balance::from(ExtrinsicBaseWeight::get().ref_time());
		smallvec![WeightToFeeCoefficient {
			degree: 1,
			negative: false,
			coeff_frac: Perbill::from_rational(p % q, q),
			coeff_integer: p / q,
		}]
	}
}

impl_opaque_keys! {
	pub struct SessionKeys {
		pub aura: Aura,
	}
}

pub type NegativeImbalanceOf<C, T> =
	<C as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

#[derive(Default)]
pub struct LiquidityInfo {
	pub imbalance: Option<NegativeImbalanceOf<Balances, Runtime>>,
	pub fee_payer: Option<<Runtime as frame_system::Config>::AccountId>,
}

pub type EnsureCouncilOrRoot = EitherOfDiverse<EnsureRoot<AccountId>, CouncilFourSeventh>;

pub type ConsensusHook = cumulus_pallet_aura_ext::FixedVelocityConsensusHook<
	Runtime,
	RELAY_CHAIN_SLOT_DURATION_MILLIS,
	BLOCK_PROCESSING_VELOCITY,
	UNINCLUDED_SEGMENT_CAPACITY,
>;
