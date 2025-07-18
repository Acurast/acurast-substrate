
//! Autogenerated weights for `pallet_acurast_marketplace`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 43.0.0
//! DATE: 2025-05-23, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `acurast-benchmark`, CPU: `AMD EPYC 7B13`
//! WASM-EXECUTION: `Compiled`, CHAIN: `Some("acurast-kusama")`, DB CACHE: 1024

// Executed Command:
// /acurast-node
// benchmark
// pallet
// --chain=acurast-kusama
// --execution=wasm
// --wasm-execution=compiled
// --pallet
// *
// --extrinsic
// *
// --steps=50
// --repeat=20
// --output=/benchmarks/

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::Weight};
use core::marker::PhantomData;

/// Weight functions for `pallet_acurast_marketplace`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> crate::WeightInfo for WeightInfo<T> {
	/// Storage: `AcurastMarketplace::StoredAdvertisementRestriction` (r:1 w:1)
	/// Proof: `AcurastMarketplace::StoredAdvertisementRestriction` (`max_values`: None, `max_size`: Some(3831), added: 6306, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredReputation` (r:1 w:1)
	/// Proof: `AcurastMarketplace::StoredReputation` (`max_values`: None, `max_size`: Some(80), added: 2555, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredStorageCapacity` (r:0 w:1)
	/// Proof: `AcurastMarketplace::StoredStorageCapacity` (`max_values`: None, `max_size`: Some(24), added: 2499, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredAdvertisementPricing` (r:0 w:1)
	/// Proof: `AcurastMarketplace::StoredAdvertisementPricing` (`max_values`: None, `max_size`: Some(73), added: 2548, mode: `MaxEncodedLen`)
	fn advertise() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `42`
		//  Estimated: `7296`
		// Minimum execution time: 22_630_000 picoseconds.
		Weight::from_parts(24_080_000, 0)
			.saturating_add(Weight::from_parts(0, 7296))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(4))
	}
	/// Storage: `AcurastMarketplace::StoredAdvertisementRestriction` (r:1 w:1)
	/// Proof: `AcurastMarketplace::StoredAdvertisementRestriction` (`max_values`: None, `max_size`: Some(3831), added: 6306, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredMatches` (r:1 w:0)
	/// Proof: `AcurastMarketplace::StoredMatches` (`max_values`: None, `max_size`: Some(345), added: 2820, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredStorageCapacity` (r:0 w:1)
	/// Proof: `AcurastMarketplace::StoredStorageCapacity` (`max_values`: None, `max_size`: Some(24), added: 2499, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredAdvertisementPricing` (r:0 w:1)
	/// Proof: `AcurastMarketplace::StoredAdvertisementPricing` (`max_values`: None, `max_size`: Some(73), added: 2548, mode: `MaxEncodedLen`)
	fn delete_advertisement() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `194`
		//  Estimated: `7296`
		// Minimum execution time: 26_960_000 picoseconds.
		Weight::from_parts(28_270_000, 0)
			.saturating_add(Weight::from_parts(0, 7296))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	/// Storage: `AcurastMarketplace::StoredMatches` (r:1 w:1)
	/// Proof: `AcurastMarketplace::StoredMatches` (`max_values`: None, `max_size`: Some(345), added: 2820, mode: `MaxEncodedLen`)
	/// Storage: `Acurast::StoredJobRegistration` (r:1 w:0)
	/// Proof: `Acurast::StoredJobRegistration` (`max_values`: None, `max_size`: Some(34817), added: 37292, mode: `MaxEncodedLen`)
	/// Storage: `Timestamp::Now` (r:1 w:0)
	/// Proof: `Timestamp::Now` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::NextReportIndex` (r:1 w:1)
	/// Proof: `AcurastMarketplace::NextReportIndex` (`max_values`: None, `max_size`: Some(126), added: 2601, mode: `MaxEncodedLen`)
	/// Storage: `AcurastProcessorManager::ProcessorToManagerIdIndex` (r:1 w:0)
	/// Proof: `AcurastProcessorManager::ProcessorToManagerIdIndex` (`max_values`: None, `max_size`: Some(32), added: 2507, mode: `MaxEncodedLen`)
	/// Storage: `Uniques::Asset` (r:1 w:0)
	/// Proof: `Uniques::Asset` (`max_values`: None, `max_size`: Some(146), added: 2621, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::JobBudgets` (r:1 w:1)
	/// Proof: `AcurastMarketplace::JobBudgets` (`max_values`: None, `max_size`: Some(32), added: 2507, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:3 w:3)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Acurast::StoredAttestation` (r:1 w:0)
	/// Proof: `Acurast::StoredAttestation` (`max_values`: None, `max_size`: Some(11623), added: 14098, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredStorageCapacity` (r:1 w:1)
	/// Proof: `AcurastMarketplace::StoredStorageCapacity` (`max_values`: None, `max_size`: Some(24), added: 2499, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::AssignedProcessors` (r:0 w:1)
	/// Proof: `AcurastMarketplace::AssignedProcessors` (`max_values`: None, `max_size`: Some(118), added: 2593, mode: `MaxEncodedLen`)
	fn report() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2057`
		//  Estimated: `38282`
		// Minimum execution time: 176_990_000 picoseconds.
		Weight::from_parts(183_150_000, 0)
			.saturating_add(Weight::from_parts(0, 38282))
			.saturating_add(T::DbWeight::get().reads(12))
			.saturating_add(T::DbWeight::get().writes(8))
	}
	/// Storage: `AcurastMarketplace::StoredJobStatus` (r:10 w:10)
	/// Proof: `AcurastMarketplace::StoredJobStatus` (`max_values`: None, `max_size`: Some(34), added: 2509, mode: `MaxEncodedLen`)
	/// Storage: `Acurast::StoredJobRegistration` (r:10 w:0)
	/// Proof: `Acurast::StoredJobRegistration` (`max_values`: None, `max_size`: Some(34817), added: 37292, mode: `MaxEncodedLen`)
	/// Storage: `Timestamp::Now` (r:1 w:0)
	/// Proof: `Timestamp::Now` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredAdvertisementRestriction` (r:640 w:0)
	/// Proof: `AcurastMarketplace::StoredAdvertisementRestriction` (`max_values`: None, `max_size`: Some(3831), added: 6306, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredAdvertisementPricing` (r:640 w:0)
	/// Proof: `AcurastMarketplace::StoredAdvertisementPricing` (`max_values`: None, `max_size`: Some(73), added: 2548, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredStorageCapacity` (r:640 w:640)
	/// Proof: `AcurastMarketplace::StoredStorageCapacity` (`max_values`: None, `max_size`: Some(24), added: 2499, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredReputation` (r:640 w:0)
	/// Proof: `AcurastMarketplace::StoredReputation` (`max_values`: None, `max_size`: Some(80), added: 2555, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredMatches` (r:1280 w:640)
	/// Proof: `AcurastMarketplace::StoredMatches` (`max_values`: None, `max_size`: Some(345), added: 2820, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredTotalAssignedV3` (r:1 w:1)
	/// Proof: `AcurastMarketplace::StoredTotalAssignedV3` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredAverageRewardV3` (r:1 w:1)
	/// Proof: `AcurastMarketplace::StoredAverageRewardV3` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::JobBudgets` (r:10 w:10)
	/// Proof: `AcurastMarketplace::JobBudgets` (`max_values`: None, `max_size`: Some(32), added: 2507, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::AssignedProcessors` (r:0 w:640)
	/// Proof: `AcurastMarketplace::AssignedProcessors` (`max_values`: None, `max_size`: Some(118), added: 2593, mode: `MaxEncodedLen`)
	/// The range of component `x` is `[1, 10]`.
	fn propose_matching(x: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `993 + x * (14903 ±0)`
		//  Estimated: `6196 + x * (403584 ±0)`
		// Minimum execution time: 2_071_234_000 picoseconds.
		Weight::from_parts(2_101_844_000, 0)
			.saturating_add(Weight::from_parts(0, 6196))
			// Standard Error: 11_001_553
			.saturating_add(Weight::from_parts(2_015_844_609, 0).saturating_mul(x.into()))
			.saturating_add(T::DbWeight::get().reads(5))
			.saturating_add(T::DbWeight::get().reads((387_u64).saturating_mul(x.into())))
			.saturating_add(T::DbWeight::get().writes(4))
			.saturating_add(T::DbWeight::get().writes((194_u64).saturating_mul(x.into())))
			.saturating_add(Weight::from_parts(0, 403584).saturating_mul(x.into()))
	}
	/// Storage: `AcurastMarketplace::StoredJobExecutionStatus` (r:10 w:20)
	/// Proof: `AcurastMarketplace::StoredJobExecutionStatus` (`max_values`: None, `max_size`: Some(96), added: 2571, mode: `MaxEncodedLen`)
	/// Storage: `Acurast::StoredJobRegistration` (r:10 w:0)
	/// Proof: `Acurast::StoredJobRegistration` (`max_values`: None, `max_size`: Some(34817), added: 37292, mode: `MaxEncodedLen`)
	/// Storage: `Timestamp::Now` (r:1 w:0)
	/// Proof: `Timestamp::Now` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::AssignedProcessors` (r:1290 w:1280)
	/// Proof: `AcurastMarketplace::AssignedProcessors` (`max_values`: None, `max_size`: Some(118), added: 2593, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredMatches` (r:2560 w:1280)
	/// Proof: `AcurastMarketplace::StoredMatches` (`max_values`: None, `max_size`: Some(345), added: 2820, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredAdvertisementRestriction` (r:640 w:0)
	/// Proof: `AcurastMarketplace::StoredAdvertisementRestriction` (`max_values`: None, `max_size`: Some(3831), added: 6306, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredAdvertisementPricing` (r:640 w:0)
	/// Proof: `AcurastMarketplace::StoredAdvertisementPricing` (`max_values`: None, `max_size`: Some(73), added: 2548, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredStorageCapacity` (r:640 w:640)
	/// Proof: `AcurastMarketplace::StoredStorageCapacity` (`max_values`: None, `max_size`: Some(24), added: 2499, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredReputation` (r:640 w:0)
	/// Proof: `AcurastMarketplace::StoredReputation` (`max_values`: None, `max_size`: Some(80), added: 2555, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::JobBudgets` (r:10 w:10)
	/// Proof: `AcurastMarketplace::JobBudgets` (`max_values`: None, `max_size`: Some(32), added: 2507, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredJobStatus` (r:0 w:10)
	/// Proof: `AcurastMarketplace::StoredJobStatus` (`max_values`: None, `max_size`: Some(34), added: 2509, mode: `MaxEncodedLen`)
	/// The range of component `x` is `[1, 10]`.
	fn propose_execution_matching(x: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1500 + x * (56955 ±0)`
		//  Estimated: `6196 + x * (721920 ±0)`
		// Minimum execution time: 4_526_508_000 picoseconds.
		Weight::from_parts(4_569_238_000, 0)
			.saturating_add(Weight::from_parts(0, 6196))
			// Standard Error: 23_476_868
			.saturating_add(Weight::from_parts(4_625_490_423, 0).saturating_mul(x.into()))
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().reads((644_u64).saturating_mul(x.into())))
			.saturating_add(T::DbWeight::get().writes(2))
			.saturating_add(T::DbWeight::get().writes((324_u64).saturating_mul(x.into())))
			.saturating_add(Weight::from_parts(0, 721920).saturating_mul(x.into()))
	}
	/// Storage: `AcurastMarketplace::StoredMatches` (r:1 w:1)
	/// Proof: `AcurastMarketplace::StoredMatches` (`max_values`: None, `max_size`: Some(345), added: 2820, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredJobStatus` (r:1 w:1)
	/// Proof: `AcurastMarketplace::StoredJobStatus` (`max_values`: None, `max_size`: Some(34), added: 2509, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::NextReportIndex` (r:0 w:1)
	/// Proof: `AcurastMarketplace::NextReportIndex` (`max_values`: None, `max_size`: Some(126), added: 2601, mode: `MaxEncodedLen`)
	fn acknowledge_match() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `522`
		//  Estimated: `3810`
		// Minimum execution time: 35_230_000 picoseconds.
		Weight::from_parts(36_140_000, 0)
			.saturating_add(Weight::from_parts(0, 3810))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	/// Storage: `AcurastMarketplace::StoredMatches` (r:1 w:1)
	/// Proof: `AcurastMarketplace::StoredMatches` (`max_values`: None, `max_size`: Some(345), added: 2820, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredJobExecutionStatus` (r:1 w:1)
	/// Proof: `AcurastMarketplace::StoredJobExecutionStatus` (`max_values`: None, `max_size`: Some(96), added: 2571, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::NextReportIndex` (r:1 w:1)
	/// Proof: `AcurastMarketplace::NextReportIndex` (`max_values`: None, `max_size`: Some(126), added: 2601, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredJobStatus` (r:0 w:1)
	/// Proof: `AcurastMarketplace::StoredJobStatus` (`max_values`: None, `max_size`: Some(34), added: 2509, mode: `MaxEncodedLen`)
	fn acknowledge_execution_match() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `582`
		//  Estimated: `3810`
		// Minimum execution time: 41_370_000 picoseconds.
		Weight::from_parts(42_080_000, 0)
			.saturating_add(Weight::from_parts(0, 3810))
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(4))
	}
	/// Storage: `AcurastMarketplace::StoredMatches` (r:1 w:1)
	/// Proof: `AcurastMarketplace::StoredMatches` (`max_values`: None, `max_size`: Some(345), added: 2820, mode: `MaxEncodedLen`)
	/// Storage: `Acurast::StoredJobRegistration` (r:1 w:0)
	/// Proof: `Acurast::StoredJobRegistration` (`max_values`: None, `max_size`: Some(34817), added: 37292, mode: `MaxEncodedLen`)
	/// Storage: `Timestamp::Now` (r:1 w:0)
	/// Proof: `Timestamp::Now` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::AssignedProcessors` (r:0 w:1)
	/// Proof: `AcurastMarketplace::AssignedProcessors` (`max_values`: None, `max_size`: Some(118), added: 2593, mode: `MaxEncodedLen`)
	fn finalize_job() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1018`
		//  Estimated: `38282`
		// Minimum execution time: 44_689_000 picoseconds.
		Weight::from_parts(45_960_000, 0)
			.saturating_add(Weight::from_parts(0, 38282))
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: `Acurast::StoredJobRegistration` (r:10 w:10)
	/// Proof: `Acurast::StoredJobRegistration` (`max_values`: None, `max_size`: Some(34817), added: 37292, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredJobStatus` (r:10 w:10)
	/// Proof: `AcurastMarketplace::StoredJobStatus` (`max_values`: None, `max_size`: Some(34), added: 2509, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::AssignedProcessors` (r:20 w:10)
	/// Proof: `AcurastMarketplace::AssignedProcessors` (`max_values`: None, `max_size`: Some(118), added: 2593, mode: `MaxEncodedLen`)
	/// Storage: `Timestamp::Now` (r:1 w:0)
	/// Proof: `Timestamp::Now` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredStorageCapacity` (r:10 w:10)
	/// Proof: `AcurastMarketplace::StoredStorageCapacity` (`max_values`: None, `max_size`: Some(24), added: 2499, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::JobBudgets` (r:10 w:10)
	/// Proof: `AcurastMarketplace::JobBudgets` (`max_values`: None, `max_size`: Some(32), added: 2507, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:2 w:2)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredMatches` (r:0 w:10)
	/// Proof: `AcurastMarketplace::StoredMatches` (`max_values`: None, `max_size`: Some(345), added: 2820, mode: `MaxEncodedLen`)
	/// The range of component `x` is `[1, 10]`.
	fn finalize_jobs(x: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `969 + x * (453 ±0)`
		//  Estimated: `6196 + x * (37292 ±0)`
		// Minimum execution time: 142_320_000 picoseconds.
		Weight::from_parts(42_086_573, 0)
			.saturating_add(Weight::from_parts(0, 6196))
			// Standard Error: 75_780
			.saturating_add(Weight::from_parts(107_472_524, 0).saturating_mul(x.into()))
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().reads((6_u64).saturating_mul(x.into())))
			.saturating_add(T::DbWeight::get().writes(2))
			.saturating_add(T::DbWeight::get().writes((6_u64).saturating_mul(x.into())))
			.saturating_add(Weight::from_parts(0, 37292).saturating_mul(x.into()))
	}
	/// Storage: `Acurast::StoredJobRegistration` (r:1 w:0)
	/// Proof: `Acurast::StoredJobRegistration` (`max_values`: None, `max_size`: Some(34817), added: 37292, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::AssignedProcessors` (r:129 w:128)
	/// Proof: `AcurastMarketplace::AssignedProcessors` (`max_values`: None, `max_size`: Some(118), added: 2593, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredMatches` (r:0 w:128)
	/// Proof: `AcurastMarketplace::StoredMatches` (`max_values`: None, `max_size`: Some(345), added: 2820, mode: `MaxEncodedLen`)
	/// The range of component `x` is `[1, 255]`.
	fn cleanup_storage(x: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2530 + x * (25 ±0)`
		//  Estimated: `84728 + x * (1303 ±29)`
		// Minimum execution time: 39_090_000 picoseconds.
		Weight::from_parts(304_195_000, 0)
			.saturating_add(Weight::from_parts(0, 84728))
			// Standard Error: 98_914
			.saturating_add(Weight::from_parts(4_384_321, 0).saturating_mul(x.into()))
			.saturating_add(T::DbWeight::get().reads(33))
			.saturating_add(T::DbWeight::get().reads((1_u64).saturating_mul(x.into())))
			.saturating_add(T::DbWeight::get().writes(63))
			.saturating_add(T::DbWeight::get().writes((1_u64).saturating_mul(x.into())))
			.saturating_add(Weight::from_parts(0, 1303).saturating_mul(x.into()))
	}
	/// Storage: `AcurastMarketplace::StoredMatches` (r:100 w:100)
	/// Proof: `AcurastMarketplace::StoredMatches` (`max_values`: None, `max_size`: Some(345), added: 2820, mode: `MaxEncodedLen`)
	/// Storage: `Acurast::StoredJobRegistration` (r:100 w:0)
	/// Proof: `Acurast::StoredJobRegistration` (`max_values`: None, `max_size`: Some(34817), added: 37292, mode: `MaxEncodedLen`)
	/// Storage: `Timestamp::Now` (r:1 w:0)
	/// Proof: `Timestamp::Now` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::AssignedProcessors` (r:0 w:100)
	/// Proof: `AcurastMarketplace::AssignedProcessors` (`max_values`: None, `max_size`: Some(118), added: 2593, mode: `MaxEncodedLen`)
	/// The range of component `x` is `[1, 100]`.
	fn cleanup_assignments(x: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `606 + x * (344 ±0)`
		//  Estimated: `1493 + x * (37292 ±0)`
		// Minimum execution time: 38_190_000 picoseconds.
		Weight::from_parts(24_694_202, 0)
			.saturating_add(Weight::from_parts(0, 1493))
			// Standard Error: 15_583
			.saturating_add(Weight::from_parts(18_225_516, 0).saturating_mul(x.into()))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().reads((2_u64).saturating_mul(x.into())))
			.saturating_add(T::DbWeight::get().writes((2_u64).saturating_mul(x.into())))
			.saturating_add(Weight::from_parts(0, 37292).saturating_mul(x.into()))
	}
	/// Storage: `Acurast::LocalJobIdSequence` (r:1 w:1)
	/// Proof: `Acurast::LocalJobIdSequence` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::Editors` (r:1 w:1)
	/// Proof: `AcurastMarketplace::Editors` (`max_values`: None, `max_size`: Some(102), added: 2577, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::DeploymentHashes` (r:1 w:1)
	/// Proof: `AcurastMarketplace::DeploymentHashes` (`max_values`: None, `max_size`: Some(64), added: 2539, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::JobKeyIds` (r:1 w:1)
	/// Proof: `AcurastMarketplace::JobKeyIds` (`max_values`: None, `max_size`: Some(102), added: 2577, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::DeploymentKeyIds` (r:1 w:1)
	/// Proof: `AcurastMarketplace::DeploymentKeyIds` (`max_values`: None, `max_size`: Some(64), added: 2539, mode: `MaxEncodedLen`)
	/// Storage: `Timestamp::Now` (r:1 w:0)
	/// Proof: `Timestamp::Now` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::StoredJobStatus` (r:1 w:1)
	/// Proof: `AcurastMarketplace::StoredJobStatus` (`max_values`: None, `max_size`: Some(34), added: 2509, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::JobBudgets` (r:1 w:1)
	/// Proof: `AcurastMarketplace::JobBudgets` (`max_values`: None, `max_size`: Some(32), added: 2507, mode: `MaxEncodedLen`)
	/// Storage: `Acurast::StoredJobRegistration` (r:0 w:1)
	/// Proof: `Acurast::StoredJobRegistration` (`max_values`: None, `max_size`: Some(34817), added: 37292, mode: `MaxEncodedLen`)
	fn deploy() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1045`
		//  Estimated: `3593`
		// Minimum execution time: 130_110_000 picoseconds.
		Weight::from_parts(133_071_000, 0)
			.saturating_add(Weight::from_parts(0, 3593))
			.saturating_add(T::DbWeight::get().reads(9))
			.saturating_add(T::DbWeight::get().writes(9))
	}
	/// Storage: `AcurastMarketplace::Editors` (r:1 w:0)
	/// Proof: `AcurastMarketplace::Editors` (`max_values`: None, `max_size`: Some(102), added: 2577, mode: `MaxEncodedLen`)
	/// Storage: `Acurast::StoredJobRegistration` (r:1 w:1)
	/// Proof: `Acurast::StoredJobRegistration` (`max_values`: None, `max_size`: Some(34817), added: 37292, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::JobKeyIds` (r:1 w:0)
	/// Proof: `AcurastMarketplace::JobKeyIds` (`max_values`: None, `max_size`: Some(102), added: 2577, mode: `MaxEncodedLen`)
	/// Storage: `AcurastMarketplace::DeploymentKeyIds` (r:1 w:2)
	/// Proof: `AcurastMarketplace::DeploymentKeyIds` (`max_values`: None, `max_size`: Some(64), added: 2539, mode: `MaxEncodedLen`)
	fn edit_script() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `822`
		//  Estimated: `38282`
		// Minimum execution time: 42_550_000 picoseconds.
		Weight::from_parts(43_560_000, 0)
			.saturating_add(Weight::from_parts(0, 38282))
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	/// Storage: `AcurastMarketplace::Editors` (r:1 w:1)
	/// Proof: `AcurastMarketplace::Editors` (`max_values`: None, `max_size`: Some(102), added: 2577, mode: `MaxEncodedLen`)
	fn transfer_editor() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `310`
		//  Estimated: `3567`
		// Minimum execution time: 18_930_000 picoseconds.
		Weight::from_parts(19_550_000, 0)
			.saturating_add(Weight::from_parts(0, 3567))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
}
