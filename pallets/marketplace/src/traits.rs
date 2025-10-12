use frame_support::{sp_runtime::FixedU128, weights::Weight};
use pallet_acurast::PoolId;

/// Trait used to lookup the time a processor was last seen, i.e. sent a heartbeat.
pub trait ProcessorInfoProvider<T: frame_system::Config + crate::Config> {
	fn last_seen(processor: &T::AccountId) -> Option<u128>;
	fn processor_version(processor: &T::AccountId) -> Option<T::ProcessorVersion>;
	fn last_processor_metric(processor: &T::AccountId, pool_id: PoolId) -> Option<FixedU128>;
}

/// Weight functions needed for pallet_acurast_marketplace.
pub trait WeightInfo {
	fn advertise() -> Weight;
	fn delete_advertisement() -> Weight;
	fn report() -> Weight;
	fn propose_matching(x: u32) -> Weight;
	fn propose_execution_matching(x: u32) -> Weight;
	fn acknowledge_match() -> Weight;
	fn acknowledge_execution_match() -> Weight;
	fn finalize_job() -> Weight;
	fn finalize_jobs(x: u32) -> Weight;
	fn cleanup_storage(x: u32) -> Weight;
	fn cleanup_assignments(x: u32) -> Weight;
	fn edit_script() -> Weight;
	fn transfer_editor() -> Weight;
	fn deploy() -> Weight;
	fn update_min_fee_per_millisecond() -> Weight;
}
