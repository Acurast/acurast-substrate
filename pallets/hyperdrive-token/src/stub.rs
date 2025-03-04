#![allow(dead_code)]

use derive_more::{From, Into};
use sp_runtime::AccountId32;
use sp_std::prelude::*;
#[cfg(not(feature = "std"))]
use sp_std::prelude::*;

pub type AccountId = AccountId32;
pub type Balance = u128;
pub type BlockNumber = u32;

// the base number of indivisible units for balances
pub const PICOUNIT: Balance = 1;
pub const NANOUNIT: Balance = 1_000;
pub const MICROUNIT: Balance = 1_000_000;
pub const MILLIUNIT: Balance = 1_000_000_000;
pub const UNIT: Balance = 1_000_000_000_000;
pub const KILOUNIT: Balance = 1_000_000_000_000_000;

pub const EXISTENTIAL_DEPOSIT: Balance = MILLIUNIT;

#[derive(Debug, From, Into, Clone, Eq, PartialEq)]
pub struct AcurastAccountId(AccountId32);
impl TryFrom<Vec<u8>> for AcurastAccountId {
	type Error = ();

	fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
		let a: [u8; 32] = value.try_into().map_err(|_| ())?;
		Ok(AcurastAccountId(AccountId32::new(a)))
	}
}

pub fn alice_account_id() -> AccountId32 {
	[0; 32].into()
}
pub fn bob_account_id() -> AccountId32 {
	[1; 32].into()
}
