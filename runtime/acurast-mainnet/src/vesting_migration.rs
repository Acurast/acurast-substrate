use frame_support::{
	sp_runtime::{traits::Convert, Weight},
	traits::{
		fungible::{Inspect, Mutate},
		tokens::Preservation,
		OnRuntimeUpgrade, VestingSchedule,
	},
};
use hex_literal::hex;
use pallet_vesting::{Config as VestingConfig, Vesting, VestingInfo};

use acurast_runtime_common::{
	constants::{DAYS, UNIT},
	types::{AccountId, Balance, BlockNumber},
	weight::RocksDbWeight,
};
use sp_runtime::traits::Zero;
use sp_std::prelude::*;

use crate::{Balances, ExtraFunds, OperationFunds, Runtime, Treasury, Vesting as VestingPallet};

const MONTH: BlockNumber = DAYS * 30;
const OLD_VESTING_START: BlockNumber = 1_359_716;
const OLD_VESTING_START_3_MONTHS: BlockNumber = OLD_VESTING_START + (3 * MONTH);
const OLD_VESTING_START_6_MONTHS: BlockNumber = OLD_VESTING_START + (6 * MONTH);

const VESTING_START: BlockNumber = 1_191_724;
const VESTING_START_3_MONTHS: BlockNumber = VESTING_START + (3 * MONTH);
const VESTING_START_6_MONTHS: BlockNumber = VESTING_START + (6 * MONTH);

const VESTING_24_MONTHS: BlockNumber = 24 * MONTH;
const VESTING_36_MONTHS: BlockNumber = 36 * MONTH;

pub struct VestingMigration;
impl OnRuntimeUpgrade for VestingMigration {
	fn on_runtime_upgrade() -> Weight {
		Self::adjust_allocations() + Self::adjust_vesting_schedules()
	}
}

impl VestingMigration {
	fn remove_vesting(account: &AccountId) -> Weight {
		let Ok(_) = VestingPallet::remove_vesting_schedule(account, 0) else {
			return Default::default();
		};

		RocksDbWeight::get().reads_writes(4, 3)
	}

	fn add_vesting(account: &AccountId, start: BlockNumber, duration: BlockNumber) -> Weight {
		let amount = Balances::total_balance(account).saturating_sub(UNIT);
		let length_as_balance: Balance =
			<Runtime as VestingConfig>::BlockNumberToBalance::convert(duration);
		let per_block = amount / length_as_balance.max(sp_runtime::traits::One::one());

		let Ok(_) = VestingPallet::add_vesting_schedule(account, amount, per_block, start) else {
			return Default::default();
		};

		RocksDbWeight::get().reads_writes(4, 3)
	}

	fn transfer(source: &AccountId, dest: &AccountId, amount: Balance) -> Weight {
		let Ok(_) = <Balances as Mutate<AccountId>>::transfer(
			source,
			dest,
			amount,
			Preservation::Expendable,
		) else {
			return Default::default();
		};

		RocksDbWeight::get().reads_writes(1, 1)
	}

	fn transfer_all(source: &AccountId, dest: &AccountId) -> Weight {
		let amount = Balances::total_balance(source);
		Self::transfer(source, dest, amount)
	}

	fn target_balance(subject: &AccountId, balance: Balance, source: &AccountId) -> Weight {
		let current_balance = Balances::total_balance(subject);
		let diff = balance.saturating_sub(current_balance);
		if !diff.is_zero() {
			Self::transfer(source, subject, diff)
		} else {
			Default::default()
		}
	}

	fn swap_accounts(
		old: &AccountId,
		new: &AccountId,
		start: BlockNumber,
		duration: BlockNumber,
	) -> Weight {
		let mut weight = Weight::zero();

		weight += Self::remove_vesting(old);
		weight += Self::transfer_all(old, new);
		weight += Self::add_vesting(new, start, duration);

		weight
	}

	fn merge_accounts(
		main: &AccountId,
		other: &AccountId,
		start: BlockNumber,
		duration: BlockNumber,
	) -> Weight {
		let mut weight = Weight::zero();

		weight += Self::remove_vesting(main);
		weight += Self::remove_vesting(other);
		weight += Self::transfer_all(other, main);
		weight += Self::add_vesting(main, start, duration);

		weight
	}

	fn adjust_allocations() -> Weight {
		let mut weight = Weight::zero();

		let acc_19_new_balance: Balance = 3_802_677 * UNIT; // check this number to know we this logic was already executed, so we avoid doing it again;

		let acc_19 = AccountId::from(hex!(
			"4831ba3d0a6cf5b083b7854cbbd86edc8acf98b82272c5eea3c38684a1834559"
		));

		let acc_19_current_balance = Balances::total_balance(&acc_19);
		weight += RocksDbWeight::get().reads(1);
		if acc_19_current_balance == acc_19_new_balance {
			// this migration logic was already executed
			return weight;
		}

		weight += Self::remove_vesting(&acc_19);

		let acc_75_1 = AccountId::from(hex!(
			"bec1e4c6cf71c6e267564cccbe2b9d825c0b68811e40f52bdb4837d49b829a41"
		));
		weight += Self::remove_vesting(&acc_75_1);

		///////////////////////////////////////////////////////////////////

		let acc_23_1 = AccountId::from(hex!(
			"b210fd61096afd0455835d2a4da7b1d0d0f3fe8b450e8722eeb9c5aae25d6e2b"
		));
		let acc_23_2 = AccountId::from(hex!(
			"16818c33cb2a52a2cc69d01cc17f71b966c5429b98171e057cc0cd45bcdaf55f"
		));
		weight += Self::merge_accounts(&acc_23_1, &acc_23_2, VESTING_START, VESTING_24_MONTHS);

		let acc_24 = AccountId::from(hex!(
			"ee2cc7f37829b104aa5950125f097215babd124b946c4b249c953c7ce24faa2a"
		));
		weight += Self::remove_vesting(&acc_24);
		weight += Self::target_balance(&acc_24, 1_153_800 * UNIT, &acc_19);
		weight += Self::add_vesting(&acc_24, VESTING_START, VESTING_24_MONTHS);

		let acc_26_old = AccountId::from(hex!(
			"0b9c59e553f2df8a18a7647876999990cff280120bb855283e07a62533aa4b5c"
		));
		let acc_26_new = AccountId::from(hex!(
			"c8eabb6a8ea439f3fe5715bf063a41d6763b70350da406ed4eb1cbd7f77d8a10"
		));
		weight += Self::swap_accounts(&acc_26_old, &acc_26_new, VESTING_START, VESTING_24_MONTHS);

		let acc_30_old = AccountId::from(hex!(
			"f829d291d692a3ab556b4d45d056c05aa40853dfcf5a44cff0923a478faf6a10"
		));
		let acc_30_new = AccountId::from(hex!(
			"e857aa4184f665ac825b432c3e3dc7762221db698de95fb663fb1ab24ecb0d3c"
		));
		weight += Self::swap_accounts(&acc_30_old, &acc_30_new, VESTING_START, VESTING_24_MONTHS);

		let acc_32_old = AccountId::from(hex!(
			"c06c4a8280caf157bcc7c481b948c9fe3fd83d6f0a1641b5337af18841622536"
		));
		let acc_32_new = AccountId::from(hex!(
			"d857c7691953383c71e23a936ac4dcd0c24c45bfe9a44025bb2579b5ecc0a52e"
		));
		weight += Self::swap_accounts(&acc_32_old, &acc_32_new, VESTING_START, VESTING_24_MONTHS);

		let acc_43_old = AccountId::from(hex!(
			"345b2c5e789d82b6c9706ecdf6c5f309d82420087ebd83b3d4d5a09a770cb119"
		));
		let acc_43_new = AccountId::from(hex!(
			"5461b518602f4e85acee70a0634cf9cd6ce2774d3a157add9d4b16e53ba09a5c"
		));
		weight += Self::swap_accounts(
			&acc_43_old,
			&acc_43_new,
			VESTING_START_6_MONTHS,
			VESTING_36_MONTHS,
		);

		let acc_44 = AccountId::from(hex!(
			"062d6528162fdee89707b54a12f99766d8b5461fbb7642f32639096bd2f67c16"
		));
		weight += Self::remove_vesting(&acc_44);
		weight += Self::target_balance(&acc_44, 866_667 * UNIT, &acc_75_1);
		weight += Self::add_vesting(&acc_44, VESTING_START_6_MONTHS, VESTING_36_MONTHS);

		let acc_48 = AccountId::from(hex!(
			"ba5bb4cf137a77af01ddb7615832c5f485a5962e36abee6941116dabcce4f42e"
		));
		weight += Self::remove_vesting(&acc_48);
		weight += Self::target_balance(&acc_48, 1_935_555 * UNIT, &acc_75_1);
		weight += Self::add_vesting(&acc_48, VESTING_START_6_MONTHS, VESTING_36_MONTHS);

		let acc_50_old = AccountId::from(hex!(
			"6e7b694b6dfe5442d6872c5cf39d7bf244d7b1c2a5ddfa2717bbfa7e1748d41a"
		));
		let acc_50_new = AccountId::from(hex!(
			"16e089b31a8c59dc7bdc597439db14717705fab1980f61621435e89588c4c86f"
		));
		weight += Self::swap_accounts(
			&acc_50_old,
			&acc_50_new,
			VESTING_START_6_MONTHS,
			VESTING_36_MONTHS,
		);

		let extra_aid = ExtraFunds::account_id();
		let target_balance = Balances::total_balance(&extra_aid).saturating_add(93_693 * UNIT);
		weight += Self::target_balance(&extra_aid, target_balance, &acc_19);
		weight += Self::add_vesting(&extra_aid, VESTING_START, VESTING_24_MONTHS);

		let treasury_aid = Treasury::account_id();
		weight += Self::remove_vesting(&treasury_aid);
		weight += Self::add_vesting(&treasury_aid, VESTING_START_3_MONTHS, VESTING_24_MONTHS);

		let operation_aid = OperationFunds::account_id();
		weight += Self::remove_vesting(&operation_aid);
		weight += Self::add_vesting(&operation_aid, VESTING_START_3_MONTHS, VESTING_24_MONTHS);

		weight += Self::add_vesting(&acc_19, VESTING_START, VESTING_24_MONTHS);
		weight += Self::add_vesting(&acc_75_1, VESTING_START_6_MONTHS, VESTING_36_MONTHS);

		weight
	}

	fn adjust_vesting_schedules() -> Weight {
		let mut weight = Weight::zero();

		let all_accounts = Vesting::<Runtime>::iter_keys().collect::<Vec<_>>();
		weight = weight.saturating_add(RocksDbWeight::get().reads(all_accounts.len() as u64));

		for account in all_accounts {
			Vesting::<Runtime>::mutate(&account, |maybe_infos| {
				weight = weight.saturating_add(RocksDbWeight::get().reads_writes(1, 1));
				let Some(infos) = maybe_infos else {
					return;
				};
				let Some(info) = infos.into_iter().nth(0) else {
					return;
				};

				let new_start: BlockNumber = match info.starting_block() {
					OLD_VESTING_START => VESTING_START,
					OLD_VESTING_START_3_MONTHS => VESTING_START_3_MONTHS,
					OLD_VESTING_START_6_MONTHS => VESTING_START_6_MONTHS,
					_ => return,
				};

				*info = VestingInfo::new(info.locked(), info.per_block(), new_start);
			});
		}

		weight
	}
}
