use frame_support::weights::{
    constants::{ExtrinsicBaseWeight, WEIGHT_PER_SECOND},
};
use polkadot_core_primitives::Balance;
pub use sp_runtime::Perbill;

pub fn base_tx_fee() -> Balance {
    super::MILLIUNIT / 10
}

pub fn default_fee_per_second() -> u128 {
    let base_weight = Balance::from(ExtrinsicBaseWeight::get());
    let base_tx_per_second = (WEIGHT_PER_SECOND as u128) / base_weight;
    base_tx_per_second * base_tx_fee()
}