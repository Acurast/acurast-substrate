use sp_runtime::Weight;

/// Weights for the pallet's dispatchables.
///
/// Only the calls that still do work appear here. The disabled ones (`convert`, `retry_convert`,
/// `retry_convert_for`, `retry_process_conversion`, `retry_process_conversion_for`) are retained in
/// the pallet purely to keep call indices stable, and each one returns `Error::NotEnabled` after the
/// origin check. They deliberately carry a fixed weight at the call site instead of an entry here:
///
/// * there is nothing to measure — they touch no storage, and
/// * they cannot be benchmarked with `#[extrinsic_call]` at all, because that expands to
///   `dispatch_bypass_filter(..)?`, so a call that always errors fails the benchmark.
///
/// Keeping unbenchmarkable methods in this trait is what made the generated weight file unusable: the
/// benchmark CLI emits one `fn` per benchmark that ran, so the file only ever implemented 2 of the 8
/// methods and could not satisfy the trait.
pub trait WeightInfo {
	fn unlock() -> Weight;
	fn set_enabled() -> Weight;
	fn deny_source() -> Weight;
}
