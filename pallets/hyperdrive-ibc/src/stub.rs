#![allow(dead_code)]

use sp_runtime::AccountId32;
#[cfg(not(feature = "std"))]
use sp_std::prelude::*;

pub type Balance = u128;

// the base number of indivisible units for balances
pub const PICOUNIT: Balance = 1;
pub const NANOUNIT: Balance = 1_000;
pub const MICROUNIT: Balance = 1_000_000;
pub const MILLIUNIT: Balance = 1_000_000_000;
pub const UNIT: Balance = 1_000_000_000_000;
pub const KILOUNIT: Balance = 1_000_000_000_000_000;

pub const EXISTENTIAL_DEPOSIT: Balance = MILLIUNIT;

pub fn alice_account_id() -> AccountId32 {
	[0; 32].into()
}
pub fn bob_account_id() -> AccountId32 {
	[1; 32].into()
}
