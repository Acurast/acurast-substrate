//! Benchmarking setup

use super::*;

#[allow(unused)]
use crate::Pallet as FeeManager;
use frame_benchmarking::{benchmarks_instance_pallet, whitelisted_caller};
use frame_system::RawOrigin;

benchmarks_instance_pallet! {
    update_fee_percentage {
        let fee_percentage = sp_arithmetic::Percent::from_percent(50);
        let caller: T::AccountId = whitelisted_caller();
    }: _(RawOrigin::Root, fee_percentage)
    verify {
        assert_eq!(Version::<T, I>::get(), 1);
        assert_eq!(FeePercentage::<T, I>::get(1), sp_arithmetic::Percent::from_percent(50));
    }

    impl_benchmark_test_suite!(FeeManager, crate::mock::new_test_ext(), crate::mock::Test);
}
