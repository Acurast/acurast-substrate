use frame_support::{
	parameter_types,
	sp_runtime::Permill,
	traits::tokens::{PayFromAccount, UnityAssetBalanceConversion},
	PalletId,
};
use frame_system::{EnsureRoot, EnsureWithSuccess};

use acurast_runtime_common::{
	constants::DAYS,
	types::{
		AccountId, Balance, BlockNumber, ExtraFundsInstance, LiquidityFundsInstance,
		OperationFundsInstance, TreasuryInstance,
	},
};

use crate::{
	Balances, EnsureCouncilOrRoot, ExtraFunds, LiquidityFunds, OperationFunds, Runtime,
	RuntimeEvent, System, Treasury,
};

parameter_types! {
	pub const SpendPeriod: BlockNumber = 7 * DAYS;
	pub const PayoutPeriod: BlockNumber = 30 * DAYS;
	pub const Burn: Permill = Permill::from_percent(0);
	pub const MaxApprovals: u32 = 100;
	pub const SpendLimit: Balance = Balance::MAX;
	pub const TreasuryPalletId: PalletId = PalletId(*b"trsrypid");
	pub TreasuryAccountId: AccountId = Treasury::account_id();
}

impl pallet_treasury::Config<TreasuryInstance> for Runtime {
	type Currency = Balances;
	type RejectOrigin = EnsureCouncilOrRoot;
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

parameter_types! {
	pub const OperationPalletId: PalletId = PalletId(*b"oprtnpid");
	pub OperationAccountId: AccountId = OperationFunds::account_id();
}

impl pallet_treasury::Config<OperationFundsInstance> for Runtime {
	type Currency = Balances;
	type RejectOrigin = EnsureCouncilOrRoot;
	type RuntimeEvent = RuntimeEvent;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
	type PalletId = OperationPalletId;
	type BurnDestination = ();
	type WeightInfo = pallet_treasury::weights::SubstrateWeight<Self>;
	type SpendFunds = ();
	type MaxApprovals = MaxApprovals;
	type SpendOrigin = EnsureWithSuccess<EnsureCouncilOrRoot, Self::AccountId, SpendLimit>;
	type AssetKind = ();
	type Beneficiary = Self::AccountId;
	type BeneficiaryLookup = Self::Lookup;
	type Paymaster = PayFromAccount<Balances, OperationAccountId>;
	type BalanceConverter = UnityAssetBalanceConversion;
	type PayoutPeriod = PayoutPeriod;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
	type BlockNumberProvider = System;
}

parameter_types! {
	pub const LiquidityPalletId: PalletId = PalletId(*b"lqdtypid");
	pub LiquidityAccountId: AccountId = LiquidityFunds::account_id();
}

impl pallet_treasury::Config<LiquidityFundsInstance> for Runtime {
	type Currency = Balances;
	type RejectOrigin = EnsureCouncilOrRoot;
	type RuntimeEvent = RuntimeEvent;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
	type PalletId = LiquidityPalletId;
	type BurnDestination = ();
	type WeightInfo = pallet_treasury::weights::SubstrateWeight<Self>;
	type SpendFunds = ();
	type MaxApprovals = MaxApprovals;
	type SpendOrigin = EnsureWithSuccess<EnsureCouncilOrRoot, Self::AccountId, SpendLimit>;
	type AssetKind = ();
	type Beneficiary = Self::AccountId;
	type BeneficiaryLookup = Self::Lookup;
	type Paymaster = PayFromAccount<Balances, LiquidityAccountId>;
	type BalanceConverter = UnityAssetBalanceConversion;
	type PayoutPeriod = PayoutPeriod;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
	type BlockNumberProvider = System;
}

parameter_types! {
	pub const ExtraPalletId: PalletId = PalletId(*b"extrapid");
	pub ExtraAccountId: AccountId = ExtraFunds::account_id();
}

impl pallet_treasury::Config<ExtraFundsInstance> for Runtime {
	type Currency = Balances;
	type RejectOrigin = EnsureCouncilOrRoot;
	type RuntimeEvent = RuntimeEvent;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
	type PalletId = ExtraPalletId;
	type BurnDestination = ();
	type WeightInfo = pallet_treasury::weights::SubstrateWeight<Self>;
	type SpendFunds = ();
	type MaxApprovals = MaxApprovals;
	type SpendOrigin = EnsureWithSuccess<EnsureCouncilOrRoot, Self::AccountId, SpendLimit>;
	type AssetKind = ();
	type Beneficiary = Self::AccountId;
	type BeneficiaryLookup = Self::Lookup;
	type Paymaster = PayFromAccount<Balances, ExtraAccountId>;
	type BalanceConverter = UnityAssetBalanceConversion;
	type PayoutPeriod = PayoutPeriod;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
	type BlockNumberProvider = System;
}
