//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 62.0.0
//! DATE: 2026-11-18 (Y/M/D)
//! HOSTNAME: `6e422ed7368c`, CPU: `AMD EPYC 4244P 6-Core Processor`
//!
//! SHORT-NAME: `block`, LONG-NAME: `BlockExecution`, RUNTIME: `acurast-parachain`
//! WARMUPS: `10`, REPEAT: `100`
//! WEIGHT-PATH: `/bench/benchmarks`
//! WEIGHT-METRIC: `Average`, WEIGHT-MUL: `1.5`, WEIGHT-ADD: `0`

// Executed Command:
//   /usr/local/bin/acurast-node
//   benchmark
//   overhead
//   --chain=/bench/benchmarks/spec.json
//   --base-path=/bench/db
//   --para-id=3396
//   --wasm-execution=compiled
//   --mul=1.5000
//   --weight-path=/bench/benchmarks

use sp_core::parameter_types;
use sp_weights::{constants::WEIGHT_REF_TIME_PER_NANOS, Weight};

parameter_types! {
	/// Weight of executing an empty block.
	/// Calculated by multiplying the *Average* with `1.5` and adding `0`.
	///
	/// Stats nanoseconds:
	///   Min, Max: 434_695, 576_321
	///   Average:  480_497
	///   Median:   474_640
	///   Std-Dev:  20659.8
	///
	/// Percentiles nanoseconds:
	///   99th: 544_430
	///   95th: 520_075
	///   75th: 487_443
	pub const BlockExecutionWeight: Weight =
		Weight::from_parts(WEIGHT_REF_TIME_PER_NANOS.saturating_mul(720_746), 5_429);
}

#[cfg(test)]
mod test_weights {
	use sp_weights::constants;

	/// Checks that the weight exists and is sane.
	// NOTE: If this test fails but you are sure that the generated values are fine,
	// you can delete it.
	#[test]
	fn sane() {
		let w = super::BlockExecutionWeight::get();

		// At least 100 µs.
		assert!(
			w.ref_time() >= 100u64 * constants::WEIGHT_REF_TIME_PER_MICROS,
			"Weight should be at least 100 µs."
		);
		// At most 50 ms.
		assert!(
			w.ref_time() <= 50u64 * constants::WEIGHT_REF_TIME_PER_MILLIS,
			"Weight should be at most 50 ms."
		);
	}
}
