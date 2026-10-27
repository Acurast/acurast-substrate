use frame_support::weights::Weight;

/// Weight functions needed for pallet_acurast_hyperdrive_ibc.
pub trait WeightInfo {
	fn update_oracles(n: u32) -> Weight;
	fn send_test_message() -> Weight;
	fn confirm_message_delivery(n: u32) -> Weight;
	fn remove_message() -> Weight;
	fn receive_message(n: u32) -> Weight;
	fn clean_incoming(x: u32) -> Weight;
}

/// Benchmark setup for the runtime's [`crate::Config::MessageProcessor`].
#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<T: crate::Config<I>, I: 'static> {
	/// Prepares the incoming message most expensive for `T::MessageProcessor` to process and
	/// returns its `(sender, recipient, payload)`, or `None` if processing never does any work.
	fn worst_case_incoming_message(
	) -> Option<(crate::SubjectFor<T>, crate::SubjectFor<T>, sp_std::vec::Vec<u8>)>;
}

#[cfg(feature = "runtime-benchmarks")]
impl<T: crate::Config<I>, I: 'static> BenchmarkHelper<T, I> for () {
	fn worst_case_incoming_message(
	) -> Option<(crate::SubjectFor<T>, crate::SubjectFor<T>, sp_std::vec::Vec<u8>)> {
		None
	}
}
