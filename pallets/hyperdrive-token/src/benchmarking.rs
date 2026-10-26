use frame_benchmarking::benchmarks_instance_pallet;
use frame_support::{
	assert_ok,
	pallet_prelude::{BoundedVec, ConstU32},
};

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
use sp_runtime::traits::{SaturatedConversion, StaticLookup};
use sp_std::prelude::*;

use super::*;

fn min_ibc_fee<T, I: 'static>() -> u128
where
	T: Config<I> + pallet_acurast_hyperdrive_ibc::Config<I>,
{
	<T as pallet_acurast_hyperdrive_ibc::Config<I>>::MinFee::get().saturated_into::<u128>()
}

fn run_to_block<T: Config<I>, I: 'static>(new_block: BlockNumberFor<T>) {
	frame_system::Pallet::<T>::set_block_number(new_block);
}

pub fn assert_last_event<T: Config<I>, I: 'static>(
	generic_event: <T as frame_system::Config>::RuntimeEvent,
) {
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
		let amount_to_transfer = UNIT;
		let fee_amount = min_ibc_fee::<T, I>().max(UNIT / 10);
		let initial_balance = (1000 * UNIT).max(fee_amount.saturating_mul(100));

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
	}: {
		assert_ok!(AcurastHyperdriveToken::<T, I>::transfer_native(RawOrigin::Signed(caller).into(), ethereum_dest().into(), amount_to_transfer.into(), fee_amount.into()));
	}

	retry_transfer_native {
		let amount_to_transfer = UNIT;
		let fee_amount = min_ibc_fee::<T, I>().max(UNIT / 10);
		let retry_fee_amount = 2 * fee_amount;
		let initial_balance = (1000 * UNIT).max(retry_fee_amount.saturating_mul(100));

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

		run_to_block::<T, I>(200u32.into());
	}: {
		assert_ok!(AcurastHyperdriveToken::<T, I>::retry_transfer_native(RawOrigin::Signed(caller).into(),
		ProxyChain::Ethereum,
		0,
		retry_fee_amount.into()));
	}

	update_ethereum_contract {
		run_to_block::<T, I>(100u32.into());

		let new_contract = AccountId20(hex!("1111111111111111111111111111111111111111"));
	}: _(RawOrigin::Root, new_contract)
	verify {
		assert_last_event::<T, I>(Event::EthereumContractUpdated { contract: new_contract }.into());
	}

	update_solana_contract {
		run_to_block::<T, I>(100u32.into());

		let new_contract = AccountId32::new([5u8; 32]);
	}: _(RawOrigin::Root, new_contract.clone())
	verify {
		assert_last_event::<T, I>(Event::SolanaContractUpdated { contract: new_contract }.into());
	}

	set_enabled {
		run_to_block::<T, I>(100u32.into());
	}: _(RawOrigin::Root, true)
	verify {
		assert_last_event::<T, I>(Event::PalletEnabled { enabled: true }.into());
	}

	enable_proxy_chain {
		let fee_amount = min_ibc_fee::<T, I>().max(UNIT / 10);
		let initial_balance = (1000 * UNIT).max(fee_amount.saturating_mul(100));

		let fee_payer: T::AccountId = T::OperationalFeeAccount::get();

		assert_ok!(AcurastHyperdriveToken::<T, I>::set_enabled(RawOrigin::Root.into(), true));

		// Arrange: initial balances and configuration
		assert_ok!(Balances::<T>::force_set_balance(RawOrigin::Root.into(), fee_payer.clone().into(), initial_balance.into()));
		assert_ok!(Balances::<T>::force_set_balance(RawOrigin::Root.into(), ethereum_vault().into(), initial_balance.into()));
		assert_ok!(Balances::<T>::force_set_balance(RawOrigin::Root.into(), ethereum_fee_vault().into(), initial_balance.into()));
		assert_ok!(AcurastHyperdriveToken::<T, I>::update_ethereum_contract(RawOrigin::Root.into(), ethereum_token_contract()));

		run_to_block::<T, I>(100u32.into());
	}: _(RawOrigin::Root, ProxyChain::Ethereum, true, fee_amount.into())

	update_outgoing_transfer_ttl {
		let ttl = 10u32;

		run_to_block::<T, I>(100u32.into());
	}: _(RawOrigin::Root, ProxyChain::Ethereum, Some(ttl.into()))

	refund_transfers {
		let x in 1 .. MAX_REFUND_TRANSFERS;

		let amount = UNIT;
		let fee_amount = min_ibc_fee::<T, I>().max(UNIT / 10);
		let initial_balance = (1000 * UNIT).max((amount + fee_amount).saturating_mul(x as u128 + 1));
		assert_ok!(AcurastHyperdriveToken::<T, I>::set_enabled(RawOrigin::Root.into(), true));
		assert_ok!(Balances::<T>::force_set_balance(RawOrigin::Root.into(), ethereum_vault().into(), initial_balance.into()));
		assert_ok!(Balances::<T>::force_set_balance(RawOrigin::Root.into(), ethereum_fee_vault().into(), initial_balance.into()));
		assert_ok!(AcurastHyperdriveToken::<T, I>::update_ethereum_contract(RawOrigin::Root.into(), ethereum_token_contract()));

		run_to_block::<T, I>(100u32.into());

		// every refund goes to a distinct account and has an expired message to remove as worst case
		let mut nonces: Vec<TransferNonce> = Vec::new();
		for i in 0..x as u64 {
			let mut raw = [7u8; 32];
			raw[..8].copy_from_slice(&i.to_le_bytes());
			let source: T::AccountId = AccountId32::new(raw).into();
			assert_ok!(Balances::<T>::force_set_balance(RawOrigin::Root.into(), source.clone().into(), (10 * (amount + fee_amount)).into()));
			nonces.push(AcurastHyperdriveToken::<T, I>::next_transfer_nonce(ProxyChain::Ethereum));
			assert_ok!(AcurastHyperdriveToken::<T, I>::transfer_native(
				RawOrigin::Signed(source).into(),
				ethereum_dest().into(),
				amount.into(),
				fee_amount.into(),
			));
		}
		let nonces: BoundedVec<TransferNonce, ConstU32<MAX_REFUND_TRANSFERS>> = nonces.try_into().unwrap();

		run_to_block::<T, I>(200u32.into());
	}: _(RawOrigin::Root, ProxyChain::Ethereum, nonces)
	verify {
		assert!(OutgoingTransfers::<T, I>::iter_prefix(ProxyChain::Ethereum).next().is_none());
	}

	remove_transfer_message {
		let amount_to_transfer = UNIT;
		let fee_amount = min_ibc_fee::<T, I>().max(UNIT / 10);
		let initial_balance = (1000 * UNIT).max(fee_amount.saturating_mul(100));

		let caller: T::AccountId = alice_account_id().into();
		whitelist_account!(caller);

		assert_ok!(AcurastHyperdriveToken::<T, I>::set_enabled(RawOrigin::Root.into(), true));

		assert_ok!(Balances::<T>::force_set_balance(RawOrigin::Root.into(), caller.clone().into(), initial_balance.into()));
		assert_ok!(Balances::<T>::force_set_balance(RawOrigin::Root.into(), ethereum_vault().into(), initial_balance.into()));
		assert_ok!(Balances::<T>::force_set_balance(RawOrigin::Root.into(), ethereum_fee_vault().into(), initial_balance.into()));
		assert_ok!(AcurastHyperdriveToken::<T, I>::update_ethereum_contract(RawOrigin::Root.into(), ethereum_token_contract()));

		run_to_block::<T, I>(100u32.into());
		assert_ok!(AcurastHyperdriveToken::<T, I>::transfer_native(
			RawOrigin::Signed(caller.clone()).into(),
			ethereum_dest().into(),
			amount_to_transfer.into(),
			fee_amount.into(),
		));

		run_to_block::<T, I>(200u32.into());
	}: _(RawOrigin::Signed(caller), ProxyChain::Ethereum, 0)

	//impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Test);
}
