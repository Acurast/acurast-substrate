//! Benchmarking setup for pallet-acurast-vesting
//!
#![cfg(feature = "runtime-benchmarks")]

use frame_benchmarking::whitelist_account;
use frame_benchmarking::whitelisted_caller;
use frame_benchmarking::{account, benchmarks_instance_pallet};
use frame_support::assert_ok;
use frame_support::traits::{OnFinalize, OnInitialize};
use frame_system::RawOrigin;
use sp_core::crypto::AccountId32;
use sp_std::prelude::*;

pub use crate::stub::*;
use crate::types::*;

use super::*;

/// Run to end block and author
fn roll_to<T: Config<I>, I: 'static>(block: u32) {
    let mut now = <frame_system::Pallet<T>>::block_number() + 1u32.into();
    while now < block.into() {
        <frame_system::Pallet<T>>::on_finalize(<frame_system::Pallet<T>>::block_number());
        <frame_system::Pallet<T>>::set_block_number(
            <frame_system::Pallet<T>>::block_number() + 1u32.into(),
        );
        <frame_system::Pallet<T>>::on_initialize(<frame_system::Pallet<T>>::block_number());
        Pallet::<T, I>::on_initialize(<frame_system::Pallet<T>>::block_number());
        now += 1u32.into();
    }
}

fn vest_helper<T: Config<I>, I: 'static>() -> (T::AccountId, VestingFor<T, I>)
where
    <T as Config<I>>::BlockNumber: From<u32>,
{
    let caller: T::AccountId = whitelisted_caller();
    whitelist_account!(caller);

    let vesting = VestingFor::<T, I> {
        stake: (10u128 * UNIT).into(),
        locking_period: 10u32.into(),
    };

    (caller, vesting)
}

benchmarks_instance_pallet! {
    where_clause {
        where
        T: Config<I>,
        <T as Config<I>>::BlockNumber: From<u32>,
        T::AccountId: From<AccountId32>,
    }

    vest {
        let (caller, vesting) = vest_helper::<T, I>();
    }: _(RawOrigin::Signed(caller.clone()), vesting.clone())
    verify {
        assert_eq!(Pallet::<T, I>::vester_states(&caller).is_some(), true);
    }

    revest {
        let (caller, vesting) = vest_helper::<T, I>();
        assert_ok!(Pallet::<T, I>::vest(RawOrigin::Signed(caller.clone()).into(), vesting.clone()));
    }: _(RawOrigin::Signed(caller.clone()), vesting.clone())
    verify {
        assert_eq!(Pallet::<T, I>::vester_states(&caller).is_some(), true);
    }

    cooldown {
        let (caller, vesting) = vest_helper::<T, I>();
        assert_ok!(Pallet::<T, I>::vest(RawOrigin::Signed(caller.clone()).into(), vesting.clone()));
    }: _(RawOrigin::Signed(caller.clone()))
    verify {
        assert_eq!(Pallet::<T, I>::vester_states(&caller).is_some(), true);
    }

    divest {
        let (caller, vesting) = vest_helper::<T, I>();
        assert_ok!(Pallet::<T, I>::vest(RawOrigin::Signed(caller.clone()).into(), vesting.clone()));
        roll_to::<T, I>(1u32);
        assert_ok!(Pallet::<T, I>::cooldown(RawOrigin::Signed(caller.clone()).into()));
        roll_to::<T, I>(11u32);
    }: _(RawOrigin::Signed(caller.clone()))
    verify {
        assert_eq!(Pallet::<T, I>::vester_states(&caller).is_some(), false);
    }

    kick_out {
        let (caller, vesting) = vest_helper::<T, I>();
        assert_ok!(Pallet::<T, I>::vest(RawOrigin::Signed(caller.clone()).into(), vesting.clone()));
        roll_to::<T, I>(1u32);
        assert_ok!(Pallet::<T, I>::cooldown(RawOrigin::Signed(caller.clone()).into()));
        roll_to::<T, I>(11u32);

        roll_to::<T, I>(40u32);

        let kicker: T::AccountId  = account::<T::AccountId>("kicker", 1, 0);
        whitelist_account!(kicker);
    }: {
        assert_ok!(Pallet::<T, I>::kick_out(RawOrigin::Signed(kicker).into(), caller.clone()));
    }
    verify {
        assert_eq!(Pallet::<T, I>::vester_states(&caller).is_some(), false);
    }

    distribute_reward {
        let (caller, vesting) = vest_helper::<T, I>();
        assert_ok!(Pallet::<T, I>::vest(RawOrigin::Signed(caller.clone()).into(), vesting.clone()));

        let s_before = <Pool<T, I>>::get().s;
    }: {
        assert_ok!(Pallet::<T, I>::distribute_reward(10u32.into()));
    }
    verify {
        let s_after = <Pool<T, I>>::get().s;
        assert!(s_after >= s_before);
    }


    impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Test);
}
