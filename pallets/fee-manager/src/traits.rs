use frame_support::weights::Weight;

/// Weight functions needed for pallet_acurast_fee_manager.
pub trait WeightInfo {
    fn update_fee_percentage() -> Weight;
}
