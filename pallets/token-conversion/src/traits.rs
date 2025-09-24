use frame_support::dispatch::DispatchResult;
use sp_runtime::Weight;

pub trait TokenConversionHook {
	fn on_initiate_conversion() -> DispatchResult;
}

impl TokenConversionHook for () {
	fn on_initiate_conversion() -> DispatchResult {
		Ok(())
	}
}

pub trait WeightInfo {
	fn convert() -> Weight;
	fn update_lock_duration() -> Weight;
	fn unlock() -> Weight;
	fn retry_convert() -> Weight;
	fn retry_convert_for() -> Weight;
	fn retry_process_conversion() -> Weight;
	fn retry_process_conversion_for() -> Weight;
	fn set_enabled() -> Weight;
}
