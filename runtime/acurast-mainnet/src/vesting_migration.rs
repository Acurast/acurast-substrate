use frame_support::traits::OnRuntimeUpgrade;
use pallet_vesting::{Vesting, VestingInfo};

use acurast_runtime_common::{constants::DAYS, types::BlockNumber, weight::RocksDbWeight};
use sp_std::prelude::*;

use crate::Runtime;

const MONTH: BlockNumber = DAYS * 30;
const OLD_VESTING_START: BlockNumber = 414_229;
const OLD_VESTING_START_6_MONTHS: BlockNumber = OLD_VESTING_START + (6 * MONTH);

const VESTING_START: BlockNumber = 527_410;
const VESTING_START_6_MONTHS: BlockNumber = VESTING_START + (6 * MONTH);

pub struct VestingMigration;
impl OnRuntimeUpgrade for VestingMigration {
	fn on_runtime_upgrade() -> sp_runtime::Weight {
		let mut weight = sp_runtime::Weight::zero();

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
					OLD_VESTING_START_6_MONTHS => VESTING_START_6_MONTHS,
					_ => return,
				};

				*info = VestingInfo::new(info.locked(), info.per_block(), new_start);
			});
		}

		weight
	}
}
