
//! Autogenerated weights for `pallet_acurast_compute`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 43.0.0
//! DATE: 2025-02-26, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `unknown8a02ead526a7.lan`, CPU: `<UNKNOWN>`
//! WASM-EXECUTION: `Compiled`, CHAIN: `Some("acurast-dev")`, DB CACHE: 1024

// Executed Command:
// ./target/release/acurast-node benchmark pallet --chain=acurast-dev --wasm-execution=compiled --pallet "pallet_acurast_compute" --extrinsic "*" --steps=50 --repeat=20 --output=./benchmarks/

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::Weight};
use core::marker::PhantomData;

/// Weight functions for `pallet_acurast_compute`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> crate::WeightInfo for WeightInfo<T> {
	/// Storage: `AcurastCompute::LastMetricPoolId` (r:1 w:1)
	/// Proof: `AcurastCompute::LastMetricPoolId` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// Storage: `AcurastCompute::MetricPoolLookup` (r:0 w:1)
	/// Proof: `AcurastCompute::MetricPoolLookup` (`max_values`: None, `max_size`: Some(25), added: 2500, mode: `MaxEncodedLen`)
	/// Storage: `AcurastCompute::MetricPools` (r:0 w:1)
	/// Proof: `AcurastCompute::MetricPools` (`max_values`: None, `max_size`: Some(1203), added: 3678, mode: `MaxEncodedLen`)
	/// The range of component `x` is `[1, 20]`.
	fn create_pool(x: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `109`
		//  Estimated: `1486`
		// Minimum execution time: 12_000_000 picoseconds.
		Weight::from_parts(12_943_514, 0)
			.saturating_add(Weight::from_parts(0, 1486))
			// Standard Error: 1_672
			.saturating_add(Weight::from_parts(1_046, 0).saturating_mul(x.into()))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	/// Storage: `AcurastCompute::MetricPools` (r:1 w:1)
	/// Proof: `AcurastCompute::MetricPools` (`max_values`: None, `max_size`: Some(1203), added: 3678, mode: `MaxEncodedLen`)
	/// Storage: `AcurastCompute::MetricPoolLookup` (r:0 w:2)
	/// Proof: `AcurastCompute::MetricPoolLookup` (`max_values`: None, `max_size`: Some(25), added: 2500, mode: `MaxEncodedLen`)
	fn modify_pool_same_config() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1381`
		//  Estimated: `4668`
		// Minimum execution time: 12_000_000 picoseconds.
		Weight::from_parts(13_000_000, 0)
			.saturating_add(Weight::from_parts(0, 4668))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	/// Storage: `AcurastCompute::MetricPools` (r:1 w:1)
	/// Proof: `AcurastCompute::MetricPools` (`max_values`: None, `max_size`: Some(1203), added: 3678, mode: `MaxEncodedLen`)
	/// Storage: `AcurastCompute::MetricPoolLookup` (r:0 w:2)
	/// Proof: `AcurastCompute::MetricPoolLookup` (`max_values`: None, `max_size`: Some(25), added: 2500, mode: `MaxEncodedLen`)
	/// The range of component `x` is `[1, 20]`.
	fn modify_pool_replace_config(_x: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1381`
		//  Estimated: `4668`
		// Minimum execution time: 12_000_000 picoseconds.
		Weight::from_parts(13_369_974, 0)
			.saturating_add(Weight::from_parts(0, 4668))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	/// Storage: `AcurastCompute::MetricPools` (r:1 w:1)
	/// Proof: `AcurastCompute::MetricPools` (`max_values`: None, `max_size`: Some(1203), added: 3678, mode: `MaxEncodedLen`)
	/// Storage: `AcurastCompute::MetricPoolLookup` (r:0 w:2)
	/// Proof: `AcurastCompute::MetricPoolLookup` (`max_values`: None, `max_size`: Some(25), added: 2500, mode: `MaxEncodedLen`)
	/// The range of component `x` is `[1, 20]`.
	fn modify_pool_update_config(x: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1381`
		//  Estimated: `4668`
		// Minimum execution time: 14_000_000 picoseconds.
		Weight::from_parts(15_095_926, 0)
			.saturating_add(Weight::from_parts(0, 4668))
			// Standard Error: 4_549
			.saturating_add(Weight::from_parts(97_412, 0).saturating_mul(x.into()))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(3))
	}
}
