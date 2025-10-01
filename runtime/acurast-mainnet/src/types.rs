use derive_more::{From, Into};
use frame_support::{
	traits::{Currency, EitherOfDiverse},
	weights::{WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial},
};
use frame_system::{EnsureRoot, EnsureSignedBy};
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
	opaque,
	types::{AccountId, Address, Balance, Signature},
	weight::ExtrinsicBaseWeight,
};

use crate::{
	AcurastProcessorManager, Admin, AllPalletsWithSystem, Aura, Balances, Runtime, RuntimeCall,
};

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

pub type TxExtension = cumulus_pallet_weight_reclaim::StorageWeightReclaim<
	Runtime,
	(
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

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
	generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, TxExtension>;

/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, RuntimeCall, TxExtension>;

/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Runtime,
	Block,
	frame_system::ChainContext<Runtime>,
	Runtime,
	AllPalletsWithSystem,
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
		// Extrinsic base weight (smallest non-zero weight) is mapped to 1 MILLIUNIT:
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

/// Extrinsic Call Filter
pub struct CallFilter;
impl frame_support::traits::Contains<RuntimeCall> for CallFilter {
	fn contains(c: &RuntimeCall) -> bool {
		match c {
			// We dont allow (non ROOT) calls to the pallet_balances while the tokenomics are not ready
			RuntimeCall::Balances(_) => false,
			RuntimeCall::Uniques(_) => false,
			_ => true,
		}
	}
}

pub type NegativeImbalanceOf<C, T> =
	<C as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

#[derive(Default)]
pub struct LiquidityInfo {
	pub imbalance: Option<NegativeImbalanceOf<Balances, Runtime>>,
	pub fee_payer: Option<<Runtime as frame_system::Config>::AccountId>,
}

// We allow root only to execute privileged collator selection operations.
pub type CollatorSelectionUpdateOrigin = EnsureAdminOrRoot;
pub type EnsureAdminOrRoot =
	EitherOfDiverse<EnsureRoot<AccountId>, EnsureSignedBy<Admin, AccountId>>;

pub type ConsensusHook = cumulus_pallet_aura_ext::FixedVelocityConsensusHook<
	Runtime,
	RELAY_CHAIN_SLOT_DURATION_MILLIS,
	BLOCK_PROCESSING_VELOCITY,
	UNINCLUDED_SEGMENT_CAPACITY,
>;
