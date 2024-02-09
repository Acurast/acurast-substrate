//! Default weights for the MMR Pallet
//! This file was not auto-generated.

use crate::NodeIndex;
use frame_support::weights::{
    constants::{RocksDbWeight as DbWeight, WEIGHT_REF_TIME_PER_NANOS},
    Weight,
};

pub trait WeightInfo {
    fn check_snapshot() -> Weight {
        let maximum_blocks_before_snapshot_reached = DbWeight::get().reads(1);
        maximum_blocks_before_snapshot_reached
    }
    fn create_snapshot() -> Weight {
        // maximum_blocks_before_snapshot_reached
        let check_weight = Self::check_snapshot();
        let message_numbers = DbWeight::get().reads_writes(1, 1);
        let next_snapshot_number = DbWeight::get().reads_writes(1, 1);
        let snapshot_root_hash = DbWeight::get().writes(1);
        let snapshot_meta = DbWeight::get().reads_writes(1, 1);
        check_weight
            .saturating_add(message_numbers)
            .saturating_add(next_snapshot_number)
            .saturating_add(snapshot_root_hash)
            .saturating_add(snapshot_meta)
    }
    fn send_message() -> Weight;
    fn send_message_actual_weight(peaks: NodeIndex) -> Weight {
        // Reading the parent hash.
        let leaf_weight = DbWeight::get().reads(1);
        // Blake2 hash cost.
        let hash_weight = Weight::from_parts(2u64 * WEIGHT_REF_TIME_PER_NANOS, 0);
        // No-op hook.
        let hook_weight = Weight::zero();

        leaf_weight
            .saturating_add(hash_weight)
            .saturating_add(hook_weight)
            .saturating_add(DbWeight::get().reads_writes(2 + peaks, 2 + peaks))
    }
}
