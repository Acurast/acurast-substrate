//! Benchmarking setup for pallet-acurast-compute

use frame_benchmarking::{benchmarks_instance_pallet, whitelist_account};
use frame_support::{assert_ok, traits::IsType};
use sp_runtime::{
	traits::{IdentifyAccount, StaticLookup, Verify},
	AccountId32, FixedU128, Perquintill,
};

use frame_system::pallet_prelude::BlockNumberFor;
use frame_system::RawOrigin;
use sp_core::*;
use sp_std::prelude::*;

use crate::{types::*, Pallet as Compute};

use crate::stub::{alice_account_id, bob_account_id, charlie_account_id, eve_account_id};
use acurast_common::{
	AttestationChain, ListUpdateOperation, MetricInput, PoolId, Version, METRICS_MAX_LENGTH,
};
use pallet_acurast_processor_manager::{
	generate_account, processor_pairing, BenchmarkHelper, Pallet as ProcessorManager,
	ProcessorPairingFor, ProcessorPairingUpdateFor,
};

use super::*;

fn generate_pairing_update_add<
	T: Config<I> + pallet_acurast_processor_manager::Config,
	I: 'static,
>(
	index: u32,
) -> ProcessorPairingUpdateFor<T>
where
	T::AccountId: From<AccountId32>,
{
	let processor_account_id = generate_account(index).into();
	let timestamp = 1657363915002u128;
	// let message = [caller.encode(), timestamp.encode(), 1u128.encode()].concat();
	let signature = <T as pallet_acurast_processor_manager::Config>::BenchmarkHelper::dummy_proof();
	ProcessorPairingUpdateFor::<T> {
		operation: ListUpdateOperation::Add,
		item: ProcessorPairingFor::<T>::new_with_proof(processor_account_id, timestamp, signature),
	}
}

fn run_to_block<T: Config<I>, I: 'static>(new_block: BlockNumberFor<T>) {
	frame_system::Pallet::<T>::set_block_number(new_block);
}

fn set_timestamp<T: pallet_timestamp::Config>(timestamp: u32) {
	pallet_timestamp::Pallet::<T>::set_timestamp(timestamp.into());
}

benchmarks_instance_pallet! {
	where_clause {
		where
		T: Config<I> + pallet_timestamp::Config<Moment = u64> + pallet_acurast_processor_manager::Config,
		BlockNumberFor<T>: From<u32>,
		T::AccountId: IsType<<<T::Proof as Verify>::Signer as IdentifyAccount>::AccountId>,
		T::AccountId: From<AccountId32> + From<[u8; 32]>,
		BalanceFor<T, I>: IsType<u128>,
		pallet_acurast_processor_manager::BalanceFor<T>: IsType<u128>,
		<<T as frame_system::Config>::Lookup as StaticLookup>::Source: From<<<T::Proof as Verify>::Signer as IdentifyAccount>::AccountId>,
	}

	create_pool {
		let x in 0 .. CONFIG_VALUES_MAX_LENGTH;
		set_timestamp::<T>(1000);
		run_to_block::<T, I>(100u32.into());

		let mut config_values = Vec::<MetricPoolConfigValue>::new();
		let c = "abcdefghijklmnopqrstuvwxyz".as_bytes();
		for i in 0..x {
			let mut config_name = *b"iterations______________";
			config_name[23] = c[i as usize];
			config_values.push((config_name.clone(), i.into(), i.into()));
		}
	}: _(RawOrigin::Root, *b"cpu-ops-per-second______", Perquintill::from_percent(20), None, config_values.try_into().unwrap())

	modify_pool_same_config {
		set_timestamp::<T>(1000);
		run_to_block::<T, I>(100u32.into());

		let c = "abcdefghijklmnopqrstuvwxyz".as_bytes();
		let mut config_values = Vec::<MetricPoolConfigValue>::new();
		for i in 0..CONFIG_VALUES_MAX_LENGTH {
			let mut config_name = *b"iterations______________";
			config_name[23] = c[i as usize];
			config_values.push((config_name.clone(), i.into(), i.into()));
		}

		assert_ok!(Compute::<T, I>::create_pool(
			RawOrigin::Root.into(),
			*b"cpu-ops-per-second______",
			Perquintill::from_percent(20),
			None,
			config_values.try_into().unwrap(),
		));
	}:  {
		assert_ok!(Compute::<T, I>::modify_pool(RawOrigin::Root.into(), 1u8, Some(*b"cpu-ops-per-second-v2___"), Some((2u32.into(), Perquintill::from_percent(30))), None, None));
	}

	modify_pool_replace_config {
		let x in 0 .. CONFIG_VALUES_MAX_LENGTH;
		set_timestamp::<T>(1000);
		run_to_block::<T, I>(100u32.into());

		let c = "abcdefghijklmnopqrstuvwxyz".as_bytes();
		let mut config_values = Vec::<MetricPoolConfigValue>::new();
		for i in 0..CONFIG_VALUES_MAX_LENGTH {
			let mut config_name = *b"iterations______________";
			config_name[23] = c[i as usize];
			config_values.push((config_name.clone(), i.into(), i.into()));
		}

		assert_ok!(Compute::<T, I>::create_pool(
			RawOrigin::Root.into(),
			*b"cpu-ops-per-second______",
			Perquintill::from_percent(20),
			None,
			config_values.try_into().unwrap(),
		));

		let mut config_values = Vec::<MetricPoolConfigValue>::new();
		for i in 0..x {
			let mut config_name = *b"iterations______________";
			config_name[23] = c[i as usize];
			config_values.push((config_name.clone(), i.into(), i.into()));
		}
		let new_config = ModifyMetricPoolConfig::Replace(config_values.try_into().unwrap());
	}:  {
		assert_ok!(Compute::<T, I>::modify_pool(RawOrigin::Root.into(), 1u8, Some(*b"cpu-ops-per-second-v2___"), Some((2u32.into(), Perquintill::from_percent(30))), None, Some(new_config)));
	}

	modify_pool_update_config {
		let x in 0 .. CONFIG_VALUES_MAX_LENGTH;
		set_timestamp::<T>(1000);
		run_to_block::<T, I>(100u32.into());

		let c = "abcdefghijklmnopqrstuvwxyz".as_bytes();
		let mut config_values = Vec::<MetricPoolConfigValue>::new();
		for i in 0..CONFIG_VALUES_MAX_LENGTH {
			let mut config_name = *b"iterations______________";
			config_name[23] = c[i as usize];
			config_values.push((config_name.clone(), i.into(), i.into()));
		}

		assert_ok!(Compute::<T, I>::create_pool(
			RawOrigin::Root.into(),
			*b"cpu-ops-per-second______",
			Perquintill::from_percent(20),
			None,
			config_values.try_into().unwrap(),
		));

		let mut config_values = Vec::<MetricPoolConfigValue>::new();
		let mut remove = Vec::<MetricPoolConfigName>::new();
		for i in 0..x {
			let mut config_name = *b"iterations______________";
			remove.push(config_name.clone());
			config_name[23] = c[i as usize];
			config_values.push((config_name.clone(), i.into(), i.into()));
		}
		let new_config = ModifyMetricPoolConfig::Update(
			MetricPoolUpdateOperations {
				add: config_values.try_into().unwrap(),
				remove: remove.try_into().unwrap(),
			}
		);
	}:  {
		assert_ok!(Compute::<T, I>::modify_pool(RawOrigin::Root.into(), 1u8, Some(*b"cpu-ops-per-second-v2___"), Some((2u32.into(), Perquintill::from_percent(30))), None, Some(new_config)));
	}

	offer_backing {
		set_timestamp::<T>(1000);
		run_to_block::<T, I>(100u32.into());

		let manager: T::AccountId = alice_account_id().into();
		let committer: T::AccountId = bob_account_id().into();

		let update = generate_pairing_update_add::<T, I>(0);
		ProcessorManager::<T>::update_processor_pairings(RawOrigin::Signed(manager.clone()).into(), vec![update.clone()].try_into().unwrap())?;
	}: _(RawOrigin::Signed(committer), manager)

	withdraw_backing_offer {
		set_timestamp::<T>(1000);
		run_to_block::<T, I>(100u32.into());

		let manager: T::AccountId = alice_account_id().into();
		let committer: T::AccountId = bob_account_id().into();

		whitelist_account!(manager);
		let update = generate_pairing_update_add::<T, I>(0);
		ProcessorManager::<T>::update_processor_pairings(RawOrigin::Signed(manager.clone()).into(), vec![update.clone()].try_into().unwrap())?;

		assert_ok!(Compute::<T, I>::offer_backing(RawOrigin::Signed(committer.clone()).into(), manager));
	}: _(RawOrigin::Signed(committer))

	accept_backing_offer {
		set_timestamp::<T>(1000);
		run_to_block::<T, I>(100u32.into());

		let manager: T::AccountId = alice_account_id().into();
		let committer: T::AccountId = bob_account_id().into();

		let update = generate_pairing_update_add::<T, I>(0);
		ProcessorManager::<T>::update_processor_pairings(RawOrigin::Signed(manager.clone()).into(), vec![update.clone()].try_into().unwrap())?;

		assert_ok!(Compute::<T, I>::offer_backing(RawOrigin::Signed(committer.clone()).into(), manager.clone()));
	}: _(RawOrigin::Signed(manager), committer)

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Test);
}
