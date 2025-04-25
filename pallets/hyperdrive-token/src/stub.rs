#![allow(dead_code)]

use derive_more::{From, Into};
use hex_literal::hex;
use pallet_acurast::{AccountId20, MultiOrigin};
use sp_core::crypto::Ss58Codec;
use sp_runtime::AccountId32;
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

pub fn ethereum_vault() -> AccountId {
	AccountId::from_ss58check("5EYCAe5h8kmzoNTSy1uhXr1rYkgpoGxKsFjxEXgW4qLN2w4M").unwrap()
}

pub fn ethereum_fee_vault() -> AccountId {
	AccountId::from_ss58check("5EYCAe5h8kmznt3JaNrvdmpvXxuTiShuZqWPN9iJm7M6iyW3").unwrap()
}

// Helper function to create MultiOrigin::Ethereum20
pub fn ethereum_dest() -> MultiOrigin<AccountId> {
	MultiOrigin::Ethereum20(AccountId20(hex!(
		"0000000000000000000000000000000000000001" // Example destination address
	)))
}

pub fn ethereum_token_contract() -> AccountId20 {
	AccountId20(hex!("7F44aD0fD6c15CfBA6f417C33924c8cF0C751d23"))
}
