//! Benchmarking setup for pallet-acurast-processor-manager
use super::*;

use frame_benchmarking::{benchmarks, whitelist_account};
use frame_system::RawOrigin;
use pallet_assets::BenchmarkHelper as AssetBenchmarkHelper;
use sp_runtime::AccountId32;
use sp_std::prelude::*;

pub trait BenchmarkHelper<T: frame_system::Config> {
    fn manager_account() -> T::AccountId;
}

impl<T> BenchmarkHelper<T> for ()
where
    T: frame_system::Config,
    T::AccountId: From<[u8; 32]>,
{
    fn manager_account() -> T::AccountId {
        return [0; 32].into();
    }
}

benchmarks! {
    where_clause { where
        T: pallet_assets::Config<Balance = u128>,
        T: frame_system::Config<AccountId = AccountId32>,
        T::AccountId: Into<<<T as frame_system::Config>::Lookup as StaticLookup>::Source>,
    }

    create {
        let caller = <T as crate::Config>::BenchmarkHelper::manager_account();
        let admin = <T as crate::Config>::BenchmarkHelper::manager_account();
        whitelist_account!(caller);
        let id = <T as pallet_assets::Config>::BenchmarkHelper::create_asset_id_parameter(0);
    }: _(RawOrigin::Signed(caller), id, xcm::latest::AssetId::Abstract([1; 32]), admin.into(), 1u128)

    force_create {
        let caller = <T as crate::Config>::BenchmarkHelper::manager_account();
        let admin = <T as crate::Config>::BenchmarkHelper::manager_account();
        whitelist_account!(caller);
        let id = <T as pallet_assets::Config>::BenchmarkHelper::create_asset_id_parameter(0);
    }: _(RawOrigin::Signed(caller), id, xcm::latest::AssetId::Abstract([1; 32]), admin.into(), true, 1u128)

    set_metadata {
        let caller = <T as crate::Config>::BenchmarkHelper::manager_account();
        whitelist_account!(caller);
        Pallet::<T>::create(RawOrigin::Signed(caller.clone()).into(), <T as pallet_assets::Config>::BenchmarkHelper::create_asset_id_parameter(0), xcm::latest::AssetId::Abstract([1; 32]), caller.clone().into(), 1u128)?;
    }: _(RawOrigin::Signed(caller), xcm::latest::AssetId::Abstract([1; 32]), vec![1u8].try_into().unwrap(), vec![1u8].try_into().unwrap(), 0)

    transfer {
        let caller = <T as crate::Config>::BenchmarkHelper::manager_account();
        let destination = <T as crate::Config>::BenchmarkHelper::manager_account();
        whitelist_account!(caller);
        let id = <T as pallet_assets::Config>::BenchmarkHelper::create_asset_id_parameter(0);
        Pallet::<T>::create(RawOrigin::Signed(caller.clone()).into(), id, xcm::latest::AssetId::Abstract([1; 32]), caller.clone().into(), 1u128)?;
        pallet_assets::Pallet::<T>::mint(RawOrigin::Signed(caller.clone()).into(), id, caller.clone().into(), 1)?;
    }: _(RawOrigin::Signed(caller), xcm::latest::AssetId::Abstract([1; 32]), destination.into(), 1)

    force_transfer {
        let caller = <T as crate::Config>::BenchmarkHelper::manager_account();
        let source = <T as crate::Config>::BenchmarkHelper::manager_account();
        let destination = <T as crate::Config>::BenchmarkHelper::manager_account();
        whitelist_account!(caller);
        let id = <T as pallet_assets::Config>::BenchmarkHelper::create_asset_id_parameter(0);
        Pallet::<T>::create(RawOrigin::Signed(caller.clone()).into(), id, xcm::latest::AssetId::Abstract([1; 32]), caller.clone().into(), 1u128)?;
        pallet_assets::Pallet::<T>::mint(RawOrigin::Signed(caller.clone()).into(), id, caller.clone().into(), 1)?;
    }: _(RawOrigin::Signed(caller), xcm::latest::AssetId::Abstract([1; 32]), source.into(), destination.into(), 1)

    impl_benchmark_test_suite!(Pallet, mock::ExtBuilder::default().build(), mock::Test);
}
