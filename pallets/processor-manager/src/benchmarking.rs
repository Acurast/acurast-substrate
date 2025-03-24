//! Benchmarking setup for pallet-acurast-processor-manager

use crate::stub::{alice_account_id, generate_account};

use super::*;

use acurast_common::{ListUpdateOperation, MetricInput, PoolId, Version};
use frame_benchmarking::{benchmarks, whitelist_account};
use frame_support::{
	sp_runtime::{
		traits::{IdentifyAccount, StaticLookup, Verify},
		AccountId32,
	},
	traits::{Get, IsType},
};
use frame_system::{pallet_prelude::BlockNumberFor, RawOrigin};
use sp_std::prelude::*;

pub trait BenchmarkHelper<T: Config> {
	fn dummy_proof() -> T::Proof;
	fn advertisement() -> T::Advertisement;
	fn funded_account(index: u32) -> T::AccountId;
	fn attest_account(account: &T::AccountId);
	fn create_compute_pool() -> PoolId;
}

fn generate_pairing_update_add<T: Config>(index: u32) -> ProcessorPairingUpdateFor<T>
where
	T::AccountId: From<AccountId32>,
{
	let processor_account_id = generate_account(index).into();
	let timestamp = 1657363915002u128;
	// let message = [caller.encode(), timestamp.encode(), 1u128.encode()].concat();
	let signature = T::BenchmarkHelper::dummy_proof();
	ProcessorPairingUpdateFor::<T> {
		operation: ListUpdateOperation::Add,
		item: ProcessorPairingFor::<T>::new_with_proof(processor_account_id, timestamp, signature),
	}
}

fn run_to_block<T: Config>(new_block: BlockNumberFor<T>) {
	frame_system::Pallet::<T>::set_block_number(new_block);
}

fn set_timestamp<T: pallet_timestamp::Config>(timestamp: u32) {
	pallet_timestamp::Pallet::<T>::set_timestamp(timestamp.into());
}

benchmarks! {
	where_clause { where
		T: Config + pallet_timestamp::Config,
		T::AccountId: IsType<<<T::Proof as Verify>::Signer as IdentifyAccount>::AccountId>,
		T::AccountId: From<AccountId32>,
		<<T as frame_system::Config>::Lookup as StaticLookup>::Source: From<<<T::Proof as Verify>::Signer as IdentifyAccount>::AccountId>,
	}

	update_processor_pairings {
		let x in 1 .. T::MaxPairingUpdates::get();
		set_timestamp::<T>(1000);
		let mut updates = Vec::<ProcessorPairingUpdateFor<T>>::new();
		let caller: T::AccountId = alice_account_id().into();
		whitelist_account!(caller);
		for i in 0..x {
			updates.push(generate_pairing_update_add::<T>(i));
		}
	}: _(RawOrigin::Signed(caller), updates.try_into().unwrap())

	pair_with_manager {
		set_timestamp::<T>(1000);
		let manager_account = generate_account(0).into();
		let processor_account = generate_account(1).into();
		let timestamp = 1657363915002u128;
		// let message = [manager_account.encode(), timestamp.encode(), 1u128.encode()].concat();
		let signature = T::BenchmarkHelper::dummy_proof();
		let item = ProcessorPairingFor::<T>::new_with_proof(manager_account, timestamp, signature);
	}: _(RawOrigin::Signed(processor_account), item)

	multi_pair_with_manager {
		set_timestamp::<T>(1000);
		let manager_account = generate_account(0).into();
		let processor_account = generate_account(1).into();
		let timestamp = 1657363915002u128;
		// let message = [manager_account.encode(), timestamp.encode(), 1u128.encode()].concat();
		let signature = T::BenchmarkHelper::dummy_proof();
		let item = ProcessorPairingFor::<T>::new_with_proof(manager_account, timestamp, signature);
	}: _(RawOrigin::Signed(processor_account), item)

	recover_funds {
		set_timestamp::<T>(1000);
		let caller: T::AccountId = alice_account_id().into();
		whitelist_account!(caller);
		let update = generate_pairing_update_add::<T>(0);
		Pallet::<T>::update_processor_pairings(RawOrigin::Signed(caller.clone()).into(), vec![update.clone()].try_into().unwrap())?;
	}: _(RawOrigin::Signed(caller.clone()), update.item.account.into().into(), caller.clone().into().into())

	heartbeat {
		set_timestamp::<T>(1000);
		let caller: T::AccountId = alice_account_id().into();
		whitelist_account!(caller);
		let update = generate_pairing_update_add::<T>(0);
		Pallet::<T>::update_processor_pairings(RawOrigin::Signed(caller.clone()).into(), vec![update.clone()].try_into().unwrap())?;
	}: _(RawOrigin::Signed(caller))

	advertise_for {
		set_timestamp::<T>(1000);
		let caller: T::AccountId = alice_account_id().into();
		whitelist_account!(caller);
		let update = generate_pairing_update_add::<T>(0);
		Pallet::<T>::update_processor_pairings(RawOrigin::Signed(caller.clone()).into(), vec![update.clone()].try_into().unwrap())?;
		let ad = T::BenchmarkHelper::advertisement();
	}: _(RawOrigin::Signed(caller), update.item.account.into().into(), ad)

	heartbeat_with_version {
		set_timestamp::<T>(1000);
		let caller: T::AccountId = alice_account_id().into();
		whitelist_account!(caller);
		T::BenchmarkHelper::attest_account(&caller);
		let distribution_settings = RewardDistributionSettings::<T::Balance, T::AccountId> {
			window_length: 1,
			tollerance: 1000,
			min_heartbeats: 1,
			reward_per_distribution: 347_222_222_222u128.into(),
			distributor_account: T::BenchmarkHelper::funded_account(0),
		};
		<ProcessorRewardDistributionWindow<T>>::insert(
			caller.clone(),
			RewardDistributionWindow::new(0, &distribution_settings),
		);
		run_to_block::<T>(100u32.into());
		Pallet::<T>::update_reward_distribution_settings(RawOrigin::Root.into(), Some(distribution_settings))?;
		let update = generate_pairing_update_add::<T>(0);
		Pallet::<T>::update_processor_pairings(RawOrigin::Signed(caller.clone()).into(), vec![update.clone()].try_into().unwrap())?;
		let version = Version {
			platform: 0,
			build_number: 1,
		};
	}: _(RawOrigin::Signed(caller), version)

	heartbeat_with_metrics {
		let x in 0 .. METRICS_MAX_LENGTH;

		set_timestamp::<T>(1000);
		let caller: T::AccountId = alice_account_id().into();
		whitelist_account!(caller);
		T::BenchmarkHelper::attest_account(&caller);
		let distribution_settings = RewardDistributionSettings::<T::Balance, T::AccountId> {
			window_length: 900,
			tollerance: 1000,
			min_heartbeats: 1,
			reward_per_distribution: 347_222_222_222u128.into(),
			distributor_account: T::BenchmarkHelper::funded_account(0),
		};
		<ProcessorRewardDistributionWindow<T>>::insert(
			caller.clone(),
			RewardDistributionWindow::new(0, &distribution_settings),
		);
		run_to_block::<T>(100u32.into());
		Pallet::<T>::update_reward_distribution_settings(RawOrigin::Root.into(), Some(distribution_settings))?;
		let update = generate_pairing_update_add::<T>(0);
		Pallet::<T>::update_processor_pairings(RawOrigin::Signed(caller.clone()).into(), vec![update.clone()].try_into().unwrap())?;
		let version = Version {
			platform: 0,
			build_number: 1,
		};

		let mut values = Vec::<MetricInput>::new();
		for i in 0..x {
			let pool_id = T::BenchmarkHelper::create_compute_pool();
			values.push((pool_id, i.into(), i.into()));
		}

		// commit initially (starting warmup)
		Pallet::<T>::heartbeat_with_metrics(RawOrigin::Signed(caller.clone()).into(), version, values.clone().try_into().unwrap())?;

		// make sure warmup of 1800 block passed
		run_to_block::<T>(1900u32.into());
		Pallet::<T>::heartbeat_with_metrics(RawOrigin::Signed(caller.clone()).into(), version, values.clone().try_into().unwrap())?;

		// make sure claim is performed by moving to next epoch, 900 blocks later
		run_to_block::<T>(2700u32.into());
	}: _(RawOrigin::Signed(caller), version, values.try_into().unwrap())

	update_binary_hash {
		set_timestamp::<T>(1000);
		let version = Version {
			platform: 0,
			build_number: 1,
		};
		let hash: BinaryHash = [1; 32].into();
	}: _(RawOrigin::Root, version, Some(hash))

	update_api_version {
		set_timestamp::<T>(1000);
		let version = 1;
	}: _(RawOrigin::Root, version)

	set_processor_update_info {
		let x in 1 .. T::MaxProcessorsInSetUpdateInfo::get();
		set_timestamp::<T>(1000);
		let caller: T::AccountId = alice_account_id().into();
		whitelist_account!(caller);
		let mut processors = Vec::<T::AccountId>::new();
		for i in 0..x {
			let update = generate_pairing_update_add::<T>(i);
			processors.push(update.item.account.clone());
			Pallet::<T>::update_processor_pairings(RawOrigin::Signed(caller.clone()).into(), vec![update.clone()].try_into().unwrap())?;
		}
		let version = Version {
			platform: 0,
			build_number: 1,
		};
		let hash: BinaryHash = [1; 32].into();
		Pallet::<T>::update_binary_hash(RawOrigin::Root.into(), version, Some(hash))?;
		let binary_location: BinaryLocation = b"https://github.com/Acurast/acurast-processor-update/releases/download/processor-1.3.31/processor-1.3.31-devnet.apk".to_vec().try_into().unwrap();
		let update_info = UpdateInfo {
			version,
			binary_location,
		};
	}: _(RawOrigin::Signed(caller), update_info, processors.try_into().unwrap())

	update_reward_distribution_settings {
		set_timestamp::<T>(1000);
		let settings = RewardDistributionSettings::<
			<T as crate::Config>::Balance,
			<T as frame_system::Config>::AccountId,
		> {
					window_length: 300,
					tollerance: 25,
					min_heartbeats: 3,
					reward_per_distribution: 300_000_000_000u128.into(),
					distributor_account: alice_account_id().into(),
		};
	}: _(RawOrigin::Root, Some(settings))

	update_min_processor_version_for_reward {
		set_timestamp::<T>(1000);
		let version = Version { platform: 0, build_number: 100 };
	}: _(RawOrigin::Root, version)

	impl_benchmark_test_suite!(Pallet, mock::ExtBuilder.build(), mock::Test);
}
