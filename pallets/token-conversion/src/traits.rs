use sp_runtime::Weight;

pub trait WeightInfo {
	fn convert() -> Weight;
	fn unlock() -> Weight;
	fn retry_convert() -> Weight;
	fn retry_convert_for() -> Weight;
	fn retry_process_conversion() -> Weight;
	fn retry_process_conversion_for() -> Weight;
	fn set_enabled() -> Weight;
}
