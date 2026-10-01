//! Benchmark helpers shared by the runtimes. Only the `impl ... for Runtime` blocks stay per
//! runtime, the orphan rule keeps them out of this crate.

use alloc::boxed::Box;
use core::marker::PhantomData;

use frame_support::{
	assert_ok,
	traits::{fungible::Mutate, Currency, Get},
};
use xcm::latest::{Asset, AssetId, Assets, Fungibility, Location, Parent};

use crate::types::{AccountId, Balance};

/// Amount of the native asset moved by the `pallet_xcm` transfer benchmarks.
fn xcm_transfer_amount<R: pallet_balances::Config<Balance = Balance>>() -> Balance {
	R::ExistentialDeposit::get() * 100
}

/// The native asset (`Location::here()`) as an XCM `Asset` of the given amount.
fn native_asset(amount: Balance) -> Asset {
	Asset { id: AssetId(Location::here()), fun: Fungibility::Fungible(amount) }
}

/// [`pallet_xcm::benchmarking::Config::reserve_transferable_asset_and_dest`].
pub fn xcm_reserve_transferable_asset_and_dest<R: pallet_balances::Config<Balance = Balance>>(
) -> Option<(Asset, Location)> {
	Some((native_asset(xcm_transfer_amount::<R>()), Parent.into()))
}

/// [`pallet_xcm::benchmarking::Config::set_up_complex_asset_transfer`].
pub fn xcm_set_up_complex_asset_transfer<R>() -> Option<(Assets, u32, Location, Box<dyn FnOnce()>)>
where
	R: pallet_balances::Config<Balance = Balance> + frame_system::Config<AccountId = AccountId>,
{
	let caller: AccountId = frame_benchmarking::whitelisted_caller();
	let amount = xcm_transfer_amount::<R>();
	let initial = amount * 10;
	pallet_balances::Pallet::<R>::make_free_balance_be(&caller, initial);

	let verify = Box::new(move || {
		assert!(pallet_balances::Pallet::<R>::free_balance(&caller) <= initial - amount);
	});

	Some((native_asset(amount).into(), 0, Parent.into(), verify))
}

/// [`pallet_xcm::benchmarking::Config::get_asset`].
pub fn xcm_get_asset<R: pallet_balances::Config<Balance = Balance>>() -> Asset {
	native_asset(R::ExistentialDeposit::get())
}

/// `pallet_treasury::Config::BenchmarkHelper` for the treasury instance paying out of `Pot`.
pub struct TreasuryArguments<R, Pot>(PhantomData<(R, Pot)>);

impl<R, Pot> pallet_treasury::ArgumentsFactory<(), AccountId> for TreasuryArguments<R, Pot>
where
	R: pallet_balances::Config<Balance = Balance> + frame_system::Config<AccountId = AccountId>,
	Pot: Get<AccountId>,
{
	fn create_asset_kind(_seed: u32) {
		endow::<R>(&Pot::get());
	}

	fn create_beneficiary(seed: [u8; 32]) -> AccountId {
		let beneficiary = AccountId::from(seed);
		endow::<R>(&beneficiary);
		beneficiary
	}
}

/// Mints the existential deposit into `who`, so a subsequent sub-ED transfer can land on it.
fn endow<R>(who: &AccountId)
where
	R: pallet_balances::Config<Balance = Balance> + frame_system::Config<AccountId = AccountId>,
{
	assert_ok!(pallet_balances::Pallet::<R>::mint_into(who, R::ExistentialDeposit::get()));
}
