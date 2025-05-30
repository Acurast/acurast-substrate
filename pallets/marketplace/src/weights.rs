
//! Autogenerated weights for `pallet_acurast_marketplace`
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
// pallet_acurast_marketplace
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

/// Weight functions for `pallet_acurast_marketplace`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> crate::WeightInfo for WeightInfo<T> {
	/// Storage: AcurastMarketplace StoredAdvertisementRestriction (r:1 w:1)
	/// Proof: AcurastMarketplace StoredAdvertisementRestriction (max_values: None, max_size: Some(3830), added: 6305, mode: MaxEncodedLen)
	/// Storage: AcurastMarketplace StoredReputation (r:1 w:1)
	/// Proof: AcurastMarketplace StoredReputation (max_values: None, max_size: Some(80), added: 2555, mode: MaxEncodedLen)
	/// Storage: AcurastMarketplace StoredStorageCapacity (r:0 w:1)
	/// Proof: AcurastMarketplace StoredStorageCapacity (max_values: None, max_size: Some(24), added: 2499, mode: MaxEncodedLen)
	/// Storage: AcurastMarketplace StoredAdvertisementPricing (r:0 w:1)
	/// Proof: AcurastMarketplace StoredAdvertisementPricing (max_values: None, max_size: Some(73), added: 2548, mode: MaxEncodedLen)
	fn advertise() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `42`
		//  Estimated: `10840`
		// Minimum execution time: 17_000_000 picoseconds.
		Weight::from_parts(17_000_000, 0)
			.saturating_add(Weight::from_parts(0, 10840))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(4))
	}
	/// Storage: AcurastMarketplace StoredAdvertisementRestriction (r:1 w:1)
	/// Proof: AcurastMarketplace StoredAdvertisementRestriction (max_values: None, max_size: Some(3830), added: 6305, mode: MaxEncodedLen)
	/// Storage: AcurastMarketplace StoredMatches (r:1 w:0)
	/// Proof: AcurastMarketplace StoredMatches (max_values: None, max_size: Some(231), added: 2706, mode: MaxEncodedLen)
	/// Storage: AcurastMarketplace StoredStorageCapacity (r:0 w:1)
	/// Proof: AcurastMarketplace StoredStorageCapacity (max_values: None, max_size: Some(24), added: 2499, mode: MaxEncodedLen)
	/// Storage: AcurastMarketplace StoredAdvertisementPricing (r:0 w:1)
	/// Proof: AcurastMarketplace StoredAdvertisementPricing (max_values: None, max_size: Some(73), added: 2548, mode: MaxEncodedLen)
	fn delete_advertisement() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `194`
		//  Estimated: `10991`
		// Minimum execution time: 18_000_000 picoseconds.
		Weight::from_parts(19_000_000, 0)
			.saturating_add(Weight::from_parts(0, 10991))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	/// Storage: AcurastMarketplace StoredMatches (r:1 w:1)
	/// Proof: AcurastMarketplace StoredMatches (max_values: None, max_size: Some(231), added: 2706, mode: MaxEncodedLen)
	/// Storage: Acurast StoredJobRegistration (r:1 w:0)
	/// Proof: Acurast StoredJobRegistration (max_values: None, max_size: Some(34795), added: 37270, mode: MaxEncodedLen)
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: AcurastProcessorManager ProcessorToManagerIdIndex (r:1 w:0)
	/// Proof: AcurastProcessorManager ProcessorToManagerIdIndex (max_values: None, max_size: Some(32), added: 2507, mode: MaxEncodedLen)
	/// Storage: Uniques Asset (r:1 w:0)
	/// Proof: Uniques Asset (max_values: None, max_size: Some(146), added: 2621, mode: MaxEncodedLen)
	/// Storage: AcurastMarketplace JobBudgets (r:1 w:1)
	/// Proof: AcurastMarketplace JobBudgets (max_values: None, max_size: Some(32), added: 2507, mode: MaxEncodedLen)
	/// Storage: AcurastFeeManager Version (r:1 w:0)
	/// Proof: AcurastFeeManager Version (max_values: Some(1), max_size: Some(2), added: 497, mode: MaxEncodedLen)
	/// Storage: AcurastFeeManager FeePercentage (r:1 w:0)
	/// Proof: AcurastFeeManager FeePercentage (max_values: None, max_size: Some(17), added: 2492, mode: MaxEncodedLen)
	/// Storage: System Account (r:3 w:3)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	fn report() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2615`
		//  Estimated: `67822`
		// Minimum execution time: 86_000_000 picoseconds.
		Weight::from_parts(87_000_000, 0)
			.saturating_add(Weight::from_parts(0, 67822))
			.saturating_add(T::DbWeight::get().reads(11))
			.saturating_add(T::DbWeight::get().writes(5))
	}
	/// Storage: AcurastMarketplace StoredJobStatus (r:10 w:10)
	/// Proof: AcurastMarketplace StoredJobStatus (max_values: None, max_size: Some(34), added: 2509, mode: MaxEncodedLen)
	/// Storage: Acurast StoredJobRegistration (r:10 w:0)
	/// Proof: Acurast StoredJobRegistration (max_values: None, max_size: Some(34795), added: 37270, mode: MaxEncodedLen)
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: AcurastMarketplace StoredAdvertisementRestriction (r:640 w:0)
	/// Proof: AcurastMarketplace StoredAdvertisementRestriction (max_values: None, max_size: Some(3830), added: 6305, mode: MaxEncodedLen)
	/// Storage: AcurastMarketplace StoredAdvertisementPricing (r:640 w:0)
	/// Proof: AcurastMarketplace StoredAdvertisementPricing (max_values: None, max_size: Some(73), added: 2548, mode: MaxEncodedLen)
	/// Storage: AcurastMarketplace StoredStorageCapacity (r:640 w:640)
	/// Proof: AcurastMarketplace StoredStorageCapacity (max_values: None, max_size: Some(24), added: 2499, mode: MaxEncodedLen)
	/// Storage: AcurastMarketplace StoredReputation (r:640 w:0)
	/// Proof: AcurastMarketplace StoredReputation (max_values: None, max_size: Some(80), added: 2555, mode: MaxEncodedLen)
	/// Storage: AcurastMarketplace StoredMatches (r:1280 w:640)
	/// Proof: AcurastMarketplace StoredMatches (max_values: None, max_size: Some(231), added: 2706, mode: MaxEncodedLen)
	/// Storage: AcurastMarketplace StoredTotalAssignedV3 (r:1 w:1)
	/// Proof: AcurastMarketplace StoredTotalAssignedV3 (max_values: Some(1), max_size: Some(16), added: 511, mode: MaxEncodedLen)
	/// Storage: AcurastMatcherFeeManager Version (r:1 w:0)
	/// Proof: AcurastMatcherFeeManager Version (max_values: Some(1), max_size: Some(2), added: 497, mode: MaxEncodedLen)
	/// Storage: AcurastMatcherFeeManager FeePercentage (r:1 w:0)
	/// Proof: AcurastMatcherFeeManager FeePercentage (max_values: None, max_size: Some(17), added: 2492, mode: MaxEncodedLen)
	/// Storage: AcurastMarketplace JobBudgets (r:10 w:10)
	/// Proof: AcurastMarketplace JobBudgets (max_values: None, max_size: Some(32), added: 2507, mode: MaxEncodedLen)
	/// Storage: AcurastFeeManager Version (r:1 w:0)
	/// Proof: AcurastFeeManager Version (max_values: Some(1), max_size: Some(2), added: 497, mode: MaxEncodedLen)
	/// Storage: AcurastFeeManager FeePercentage (r:1 w:0)
	/// Proof: AcurastFeeManager FeePercentage (max_values: None, max_size: Some(17), added: 2492, mode: MaxEncodedLen)
	/// Storage: System Account (r:2 w:2)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// Storage: AcurastMarketplace AssignedProcessors (r:0 w:640)
	/// Proof: AcurastMarketplace AssignedProcessors (max_values: None, max_size: Some(118), added: 2593, mode: MaxEncodedLen)
	/// The range of component `x` is `[1, 10]`.
	fn propose_matching(x: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1021 + x * (14901 ±0)`
		//  Estimated: `27048 + x * (1278702 ±0)`
		// Minimum execution time: 1_472_000_000 picoseconds.
		Weight::from_parts(1_482_000_000, 0)
			.saturating_add(Weight::from_parts(0, 27048))
			// Standard Error: 8_200_029
			.saturating_add(Weight::from_parts(1_476_890_801, 0).saturating_mul(x.into()))
			.saturating_add(T::DbWeight::get().reads(8))
			.saturating_add(T::DbWeight::get().reads((387_u64).saturating_mul(x.into())))
			.saturating_add(T::DbWeight::get().writes(3))
			.saturating_add(T::DbWeight::get().writes((194_u64).saturating_mul(x.into())))
			.saturating_add(Weight::from_parts(0, 1278702).saturating_mul(x.into()))
	}
	fn propose_execution_matching(x: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1021 + x * (14901 ±0)`
		//  Estimated: `27048 + x * (1278702 ±0)`
		// Minimum execution time: 1_472_000_000 picoseconds.
		Weight::from_parts(1_482_000_000, 0)
			.saturating_add(Weight::from_parts(0, 27048))
			// Standard Error: 8_200_029
			.saturating_add(Weight::from_parts(1_476_890_801, 0).saturating_mul(x.into()))
			.saturating_add(T::DbWeight::get().reads(8))
			.saturating_add(T::DbWeight::get().reads((387_u64).saturating_mul(x.into())))
			.saturating_add(T::DbWeight::get().writes(3))
			.saturating_add(T::DbWeight::get().writes((194_u64).saturating_mul(x.into())))
			.saturating_add(Weight::from_parts(0, 1278702).saturating_mul(x.into()))
	}
	/// Storage: AcurastMarketplace StoredMatches (r:1 w:1)
	/// Proof: AcurastMarketplace StoredMatches (max_values: None, max_size: Some(231), added: 2706, mode: MaxEncodedLen)
	/// Storage: AcurastMarketplace StoredJobStatus (r:1 w:1)
	/// Proof: AcurastMarketplace StoredJobStatus (max_values: None, max_size: Some(34), added: 2509, mode: MaxEncodedLen)
	fn acknowledge_match() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `488`
		//  Estimated: `7195`
		// Minimum execution time: 21_000_000 picoseconds.
		Weight::from_parts(22_000_000, 0)
			.saturating_add(Weight::from_parts(0, 7195))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	fn acknowledge_execution_match() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `488`
		//  Estimated: `7195`
		// Minimum execution time: 21_000_000 picoseconds.
		Weight::from_parts(22_000_000, 0)
			.saturating_add(Weight::from_parts(0, 7195))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: Acurast StoredJobRegistration (r:1 w:0)
	/// Proof: Acurast StoredJobRegistration (max_values: None, max_size: Some(34795), added: 37270, mode: MaxEncodedLen)
	/// Storage: AcurastMarketplace StoredMatches (r:1 w:1)
	/// Proof: AcurastMarketplace StoredMatches (max_values: None, max_size: Some(231), added: 2706, mode: MaxEncodedLen)
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: Acurast StoredAttestation (r:1 w:0)
	/// Proof: Acurast StoredAttestation (max_values: None, max_size: Some(11622), added: 14097, mode: MaxEncodedLen)
	/// Storage: AcurastMarketplace StoredStorageCapacity (r:1 w:1)
	/// Proof: AcurastMarketplace StoredStorageCapacity (max_values: None, max_size: Some(24), added: 2499, mode: MaxEncodedLen)
	/// Storage: AcurastMarketplace AssignedProcessors (r:0 w:1)
	/// Proof: AcurastMarketplace AssignedProcessors (max_values: None, max_size: Some(118), added: 2593, mode: MaxEncodedLen)
	fn finalize_job() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1102`
		//  Estimated: `62025`
		// Minimum execution time: 36_000_000 picoseconds.
		Weight::from_parts(37_000_000, 0)
			.saturating_add(Weight::from_parts(0, 62025))
			.saturating_add(T::DbWeight::get().reads(5))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	/// Storage: AcurastMarketplace StoredJobStatus (r:10 w:10)
	/// Proof: AcurastMarketplace StoredJobStatus (max_values: None, max_size: Some(34), added: 2509, mode: MaxEncodedLen)
	/// Storage: Acurast StoredJobRegistration (r:10 w:10)
	/// Proof: Acurast StoredJobRegistration (max_values: None, max_size: Some(34795), added: 37270, mode: MaxEncodedLen)
	/// Storage: AcurastMarketplace AssignedProcessors (r:20 w:10)
	/// Proof: AcurastMarketplace AssignedProcessors (max_values: None, max_size: Some(118), added: 2593, mode: MaxEncodedLen)
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: AcurastMarketplace StoredStorageCapacity (r:10 w:10)
	/// Proof: AcurastMarketplace StoredStorageCapacity (max_values: None, max_size: Some(24), added: 2499, mode: MaxEncodedLen)
	/// Storage: AcurastMarketplace JobBudgets (r:10 w:10)
	/// Proof: AcurastMarketplace JobBudgets (max_values: None, max_size: Some(32), added: 2507, mode: MaxEncodedLen)
	/// Storage: AcurastMarketplace StoredMatches (r:0 w:10)
	/// Proof: AcurastMarketplace StoredMatches (max_values: None, max_size: Some(231), added: 2706, mode: MaxEncodedLen)
	/// The range of component `x` is `[1, 10]`.
	fn finalize_jobs(x: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `657 + x * (431 ±0)`
		//  Estimated: `6443 + x * (49971 ±0)`
		// Minimum execution time: 58_000_000 picoseconds.
		Weight::from_parts(13_348_701, 0)
			.saturating_add(Weight::from_parts(0, 6443))
			// Standard Error: 62_399
			.saturating_add(Weight::from_parts(47_230_935, 0).saturating_mul(x.into()))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().reads((6_u64).saturating_mul(x.into())))
			.saturating_add(T::DbWeight::get().writes((6_u64).saturating_mul(x.into())))
			.saturating_add(Weight::from_parts(0, 49971).saturating_mul(x.into()))
	}

	fn cleanup_storage(x: u32) -> Weight {
		Weight::from_parts(13_348_701, 0)
			.saturating_add(Weight::from_parts(0, 6443))
			// Standard Error: 62_399
			.saturating_add(Weight::from_parts(47_230_935, 0).saturating_mul(x.into()))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().reads((6_u64).saturating_mul(x.into())))
			.saturating_add(T::DbWeight::get().writes((6_u64).saturating_mul(x.into())))
			.saturating_add(Weight::from_parts(0, 49971).saturating_mul(x.into()))
	}

	fn cleanup_assignments(x: u32, ) -> Weight {
		Weight::from_parts(24_380_858, 0)
			.saturating_add(Weight::from_parts(0, 1493))
			// Standard Error: 12_405
			.saturating_add(Weight::from_parts(15_736_544, 0).saturating_mul(x.into()))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().reads((2_u64).saturating_mul(x.into())))
			.saturating_add(T::DbWeight::get().writes((1_u64).saturating_mul(x.into())))
			.saturating_add(Weight::from_parts(0, 37292).saturating_mul(x.into()))
	}
}
