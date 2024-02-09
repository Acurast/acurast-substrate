//! Benchmarks for the MMR pallet.

#![cfg(feature = "runtime-benchmarks")]

use frame_benchmarking::benchmarks_instance_pallet;

use crate::stub::action;
use crate::*;

benchmarks_instance_pallet! {
    send_message {
        let x in 1 .. 1_000;

        let leaves = x as NodeIndex;
        for i in 0..leaves {
            _ = Pallet::<T, I>::send_message(action(i as u128));
        }
    }: {
        // insert last leave as the benchmarked one
        _ = Pallet::<T, I>::send_message(action(leaves as u128));
    } verify {
        assert_eq!(crate::NumberOfLeaves::<T, I>::get(), leaves+1);
    }

    impl_benchmark_test_suite!(Pallet, crate::tests::new_test_ext(), crate::mock::Test);
}
