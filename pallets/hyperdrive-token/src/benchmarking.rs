use frame_benchmarking::benchmarks_instance_pallet;
use frame_support::assert_ok;

pub use crate::stub::*;
use crate::Pallet as AcurastHyperdriveToken;
use frame_benchmarking::whitelist_account;
use frame_system::pallet_prelude::BlockNumberFor;
use frame_system::RawOrigin;
use hex_literal::hex;
use pallet_acurast::{AccountId20, MultiOrigin, ProxyChain};
use pallet_balances::Pallet as Balances;
use sp_core::crypto::AccountId32;
use sp_core::*;
use sp_runtime::traits::StaticLookup;
use sp_std::prelude::*;

use super::*;

fn run_to_block<T: Config<I>, I: 'static>(new_block: BlockNumberFor<T>) {
	frame_system::Pallet::<T>::set_block_number(new_block);
}

pub fn assert_last_event<T: Config<I>, I: 'static>(generic_event: <T as Config<I>>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

benchmarks_instance_pallet! {
	where_clause {
		where
		T: Config<I> + pallet_acurast_hyperdrive_ibc::Config<I> + pallet_balances::Config,
		MultiOrigin<T::AccountId>: From<MultiOrigin<AccountId32>>,
		T::AccountId: From<AccountId32>,
		<T as pallet_balances::Config>::Balance: From<u128>,
		<<T as frame_system::Config>::Lookup as StaticLookup>::Source: From<AccountId32> + From<T::AccountId>,
	}

	transfer_native {
		let initial_balance = 1000 * UNIT;
		let amount_to_transfer = 1 * UNIT;
		let fee_amount = 2 * MILLIUNIT;

		let caller: T::AccountId = alice_account_id().into();
		whitelist_account!(caller);

		assert_ok!(AcurastHyperdriveToken::<T, I>::set_enabled(RawOrigin::Root.into(), true));

		// Arrange: initial balances and configuration
		assert_ok!(
			Balances::<T>::force_set_balance(RawOrigin::Root.into(), caller.clone().into(), initial_balance.into()));
			assert_ok!(
			Balances::<T>::force_set_balance(RawOrigin::Root.into(), ethereum_vault().into(), initial_balance.into()));
			assert_ok!(Balances::<T>::force_set_balance(
			RawOrigin::Root.into(),
			ethereum_fee_vault().into(),
			initial_balance.into(),
		));
		assert_ok!(AcurastHyperdriveToken::<T, I>::update_ethereum_contract(RawOrigin::Root.into(), ethereum_token_contract()));

		let amount_to_transfer = 1 * UNIT;
		let fee_amount = 2 * MILLIUNIT;

		run_to_block::<T, I>(100u32.into());
	}: {
		assert_ok!(AcurastHyperdriveToken::<T, I>::transfer_native(RawOrigin::Signed(caller).into(), ethereum_dest().into(), amount_to_transfer.into(), fee_amount.into()));
	}

	retry_transfer_native {
		let initial_balance = 1000 * UNIT;
		let amount_to_transfer = 1 * UNIT;
		let fee_amount = 2 * MILLIUNIT;
		let retry_fee_amount = 3 * MILLIUNIT;

		let caller: T::AccountId = alice_account_id().into();
		whitelist_account!(caller);

		assert_ok!(AcurastHyperdriveToken::<T, I>::set_enabled(RawOrigin::Root.into(), true));

		// Arrange: initial balances and configuration
		assert_ok!(
			Balances::<T>::force_set_balance(RawOrigin::Root.into(), caller.clone().into(), initial_balance.into()));
			assert_ok!(
			Balances::<T>::force_set_balance(RawOrigin::Root.into(), ethereum_vault().into(), initial_balance.into()));
			assert_ok!(Balances::<T>::force_set_balance(
			RawOrigin::Root.into(),
			ethereum_fee_vault().into(),
			initial_balance.into(),
		));
		assert_ok!(AcurastHyperdriveToken::<T, I>::update_ethereum_contract(RawOrigin::Root.into(), ethereum_token_contract()));

		run_to_block::<T, I>(100u32.into());
		assert_ok!(AcurastHyperdriveToken::<T, I>::transfer_native(
			RawOrigin::Signed(caller.clone()).into(),
			ethereum_dest().into(),
			amount_to_transfer.into(),
			fee_amount.into(),
		));

		run_to_block::<T, I>(116u32.into());
	}: {

		assert_ok!(AcurastHyperdriveToken::<T, I>::retry_transfer_native(RawOrigin::Signed(caller).into(),
		ProxyChain::Ethereum,
		0,
		retry_fee_amount.into()));
	}

	update_ethereum_contract {
		run_to_block::<T, I>(100u32.into());

		let new_contract = AccountId20(hex!("1111111111111111111111111111111111111111"));
	}: _(RawOrigin::Root,
		new_contract.clone())
	verify {
		assert_last_event::<T, I>(Event::EthereumContractUpdated { contract: new_contract.into() }.into());
	}

	update_solana_contract {
		run_to_block::<T, I>(100u32.into());

		let new_contract = AccountId32::new([5u8; 32]);
	}: _(RawOrigin::Root,
		new_contract.clone())
	verify {
		assert_last_event::<T, I>(Event::SolanaContractUpdated { contract: new_contract.into() }.into());
	}

	set_enabled {
		run_to_block::<T, I>(100u32.into());
	}: _(RawOrigin::Root, true)
	verify {
		assert_last_event::<T, I>(Event::PalletEnabled { enabled: true }.into());
	}

	enable_proxy_chain {
		let initial_balance = 1000 * UNIT;
		let fee_amount = 2 * MILLIUNIT;

		let fee_payer: T::AccountId = T::OperationalFeeAccount::get();

		assert_ok!(AcurastHyperdriveToken::<T, I>::set_enabled(RawOrigin::Root.into(), true));

		// Arrange: initial balances and configuration
		assert_ok!(
			Balances::<T>::force_set_balance(RawOrigin::Root.into(), fee_payer.clone().into(), initial_balance.into()));
			assert_ok!(
			Balances::<T>::force_set_balance(RawOrigin::Root.into(), ethereum_vault().into(), initial_balance.into()));
			assert_ok!(Balances::<T>::force_set_balance(
			RawOrigin::Root.into(),
			ethereum_fee_vault().into(),
			initial_balance.into(),
		));
		assert_ok!(AcurastHyperdriveToken::<T, I>::update_ethereum_contract(RawOrigin::Root.into(), ethereum_token_contract()));

		run_to_block::<T, I>(100u32.into());
	}: _(RawOrigin::Root, ProxyChain::Ethereum, true, fee_amount.into())

	//impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Test);
}
