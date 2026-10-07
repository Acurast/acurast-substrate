//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 58.0.0
//! DATE: 2026-10-07 (Y/M/D)
//! HOSTNAME: `1fbbe1a025d8`, CPU: `AMD EPYC 4244P 6-Core Processor`
//!
//! SHORT-NAME: `extrinsic`, LONG-NAME: `ExtrinsicBase`, RUNTIME: `acurast-parachain`
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
	/// Weight of executing a NO-OP extrinsic, for example `System::remark`.
	/// Calculated by multiplying the *Average* with `1.5` and adding `0`.
	///
	/// Stats nanoseconds:
	///   Min, Max: 83_456, 102_269
	///   Average:  86_756
	///   Median:   85_708
	///   Std-Dev:  3143.91
	///
	/// Percentiles nanoseconds:
	///   99th: 96_314
	///   95th: 92_668
	///   75th: 87_612
	pub const ExtrinsicBaseWeight: Weight =
		Weight::from_parts(WEIGHT_REF_TIME_PER_NANOS.saturating_mul(130_134), 368);
}

#[cfg(test)]
mod test_weights {
	use sp_weights::constants;

	/// Checks that the weight exists and is sane.
	// NOTE: If this test fails but you are sure that the generated values are fine,
	// you can delete it.
	#[test]
	fn sane() {
		let w = super::ExtrinsicBaseWeight::get();

		// At least 10 µs.
		assert!(
			w.ref_time() >= 10u64 * constants::WEIGHT_REF_TIME_PER_MICROS,
			"Weight should be at least 10 µs."
		);
		// At most 1 ms.
		assert!(
			w.ref_time() <= constants::WEIGHT_REF_TIME_PER_MILLIS,
			"Weight should be at most 1 ms."
		);
	}
}
