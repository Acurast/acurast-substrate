//! Benchmarking setup for pallet-acurast-compute
#![cfg(feature = "runtime-benchmarks")]

use frame_benchmarking::benchmarks_instance_pallet;
use frame_support::assert_ok;
use sp_runtime::Perquintill;

use frame_system::pallet_prelude::BlockNumberFor;
use frame_system::RawOrigin;
use sp_core::crypto::AccountId32;
use sp_core::*;
use sp_std::prelude::*;

use crate::types::*;
use crate::Pallet as Compute;

use super::*;

fn run_to_block<T: Config<I>, I: 'static>(new_block: BlockNumberFor<T>) {
	frame_system::Pallet::<T>::set_block_number(new_block);
}

fn set_timestamp<T: pallet_timestamp::Config>(timestamp: u32) {
	pallet_timestamp::Pallet::<T>::set_timestamp(timestamp.into());
}

benchmarks_instance_pallet! {
	where_clause {
		where
		T: Config<I> + pallet_timestamp::Config,
		<T as Config<I>>::BlockNumber: From<u32>,
		T::AccountId: From<AccountId32>,
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
	}: _(RawOrigin::Root, *b"cpu-ops-per-second______", Perquintill::from_percent(20), config_values.try_into().unwrap())

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
			config_values.try_into().unwrap(),
		));
	}:  {
		assert_ok!(Compute::<T, I>::modify_pool(RawOrigin::Root.into(), 1u8, Some(*b"cpu-ops-per-second-v2___"), Some((2u32.into(), Perquintill::from_percent(30))), None));
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
		assert_ok!(Compute::<T, I>::modify_pool(RawOrigin::Root.into(), 1u8, Some(*b"cpu-ops-per-second-v2___"), Some((2u32.into(), Perquintill::from_percent(30))), Some(new_config)));
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
		assert_ok!(Compute::<T, I>::modify_pool(RawOrigin::Root.into(), 1u8, Some(*b"cpu-ops-per-second-v2___"), Some((2u32.into(), Perquintill::from_percent(30))), Some(new_config)));
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Test);
}
