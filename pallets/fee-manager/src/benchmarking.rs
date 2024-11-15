//! Benchmarking setup

use super::*;

#[allow(unused)]
use crate::Pallet as FeeManager;
use frame_benchmarking::{benchmarks_instance_pallet, whitelisted_caller};
use frame_system::RawOrigin;

fn set_timestamp<T: pallet_timestamp::Config>(timestamp: u32) {
	pallet_timestamp::Pallet::<T>::set_timestamp(timestamp.into());
}

benchmarks_instance_pallet! {
	where_clause { where
		T: pallet_timestamp::Config,
	}
	update_fee_percentage {
		set_timestamp::<T>(1000);
		let fee_percentage = sp_arithmetic::Percent::from_percent(50);
		let caller: T::AccountId = whitelisted_caller();
	}: _(RawOrigin::Root, fee_percentage)
	verify {
		assert_eq!(Version::<T, I>::get(), 1);
		assert_eq!(FeePercentage::<T, I>::get(1), sp_arithmetic::Percent::from_percent(50));
	}

	impl_benchmark_test_suite!(FeeManager, crate::mock::new_test_ext(), crate::mock::Test);
}
