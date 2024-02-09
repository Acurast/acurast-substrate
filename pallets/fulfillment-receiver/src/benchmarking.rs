use super::*;
use crate::stub::bob_account_id;

use frame_benchmarking::{benchmarks, whitelist_account};
use frame_support::sp_runtime::AccountId32;
use frame_system::RawOrigin;
use hex_literal::hex;

benchmarks! {
    where_clause { where
        T: Config<AccountId = AccountId32>
    }

    fulfill {
        let caller: T::AccountId = bob_account_id().into();
        whitelist_account!(caller);
        let fulfillment = Fulfillment {
            script: hex!("697066733A2F2F00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").to_vec().try_into().unwrap(),
            payload: hex!("00000000").to_vec(),
        };
    }: _(RawOrigin::Signed(caller), fulfillment)

    impl_benchmark_test_suite!(Pallet, mock::ExtBuilder::default().build(), mock::Test);
}
