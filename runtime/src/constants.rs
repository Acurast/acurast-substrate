use frame_support::weights::constants::{ExtrinsicBaseWeight, WEIGHT_REF_TIME_PER_SECOND};
use polkadot_core_primitives::Balance;
pub use sp_runtime::Perbill;

/// Returns the base transaction fee.
pub fn base_tx_fee() -> Balance {
	super::MILLIUNIT / 10
}

/// Returns the default fee per second.
pub fn default_fee_per_second() -> u128 {
	let base_weight = Balance::from(ExtrinsicBaseWeight::get().ref_time());
	let base_tx_per_second = (WEIGHT_REF_TIME_PER_SECOND as u128) / base_weight;
	base_tx_per_second * base_tx_fee()
}

/// The tezos target chain instance.
pub type TargetChainTezos = pallet_acurast_hyperdrive_outgoing::Instance1;
pub const INDEXING_PREFIX: &'static [u8] = b"mmr-tez-";
pub const TEMP_INDEXING_PREFIX: &'static [u8] = b"mmr-tez-temp-";
