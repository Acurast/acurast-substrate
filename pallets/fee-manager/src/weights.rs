
//! Autogenerated weights for `pallet_acurast_fee_manager`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2023-07-21, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `jenova`, CPU: `<UNKNOWN>`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("acurast-dev"), DB CACHE: 1024

// Executed Command:
// ./target/release/acurast-node
// benchmark
// pallet
// --chain=acurast-dev
// --execution=wasm
// --wasm-execution=compiled
// --pallet
// pallet_acurast_fee_manager
// --extrinsic
// *
// --steps=50
// --repeat=20
// --output=./benchmarks/

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

/// Weight functions for `pallet_acurast_fee_manager`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> crate::WeightInfo for WeightInfo<T> {
	/// Storage: AcurastFeeManager Version (r:1 w:1)
	/// Proof: AcurastFeeManager Version (max_values: Some(1), max_size: Some(2), added: 497, mode: MaxEncodedLen)
	/// Storage: AcurastFeeManager FeePercentage (r:0 w:1)
	/// Proof: AcurastFeeManager FeePercentage (max_values: None, max_size: Some(17), added: 2492, mode: MaxEncodedLen)
	fn update_fee_percentage() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `42`
		//  Estimated: `1487`
		// Minimum execution time: 10_000_000 picoseconds.
		Weight::from_parts(11_000_000, 0)
			.saturating_add(Weight::from_parts(0, 1487))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(2))
	}
}
