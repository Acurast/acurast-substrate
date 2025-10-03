use acurast_runtime_common::{
	constants::DAYS,
	types::{AccountId, Balance, BlockNumber},
};
use frame_support::{
	parameter_types,
	traits::tokens::{PayFromAccount, UnityAssetBalanceConversion},
	PalletId,
};
use frame_system::{EnsureRoot, EnsureWithSuccess};
use sp_runtime::Permill;

use crate::{Balances, Runtime, RuntimeEvent, System, Treasury};

parameter_types! {
	pub const SpendPeriod: BlockNumber = 7 * DAYS;
	pub const PayoutPeriod: BlockNumber = 30 * DAYS;
	pub const Burn: Permill = Permill::from_percent(0);
	pub const MaxApprovals: u32 = 100;
	pub const SpendLimit: Balance = Balance::MAX;
	pub const TreasuryPalletId: PalletId = PalletId(*b"trsrypid");
	pub TreasuryAccountId: AccountId = Treasury::account_id();
}

impl pallet_treasury::Config for Runtime {
	type Currency = Balances;
	type RejectOrigin = EnsureRoot<Self::AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
	type PalletId = TreasuryPalletId;
	type BurnDestination = ();
	type WeightInfo = pallet_treasury::weights::SubstrateWeight<Self>;
	type SpendFunds = ();
	type MaxApprovals = MaxApprovals;
	type SpendOrigin = EnsureWithSuccess<EnsureRoot<Self::AccountId>, Self::AccountId, SpendLimit>;
	type AssetKind = ();
	type Beneficiary = Self::AccountId;
	type BeneficiaryLookup = Self::Lookup;
	type Paymaster = PayFromAccount<Balances, TreasuryAccountId>;
	type BalanceConverter = UnityAssetBalanceConversion;
	type PayoutPeriod = PayoutPeriod;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
	type BlockNumberProvider = System;
}
