use frame_support::{parameter_types, traits::tokens::imbalance::ResolveTo, PalletId};
use frame_system::EnsureRoot;
use polkadot_core_primitives::BlakeTwo256;
use sp_runtime::traits::AccountIdConversion;

use acurast_runtime_common::{
	constants::{CanaryTokenConversionPalletId, MainnetTokenConversionPalletId, DAYS, UNIT},
	types::{AccountId, Balance, BlockNumber},
};
use pallet_acurast::{Layer, ProxyChain, Subject};
use pallet_acurast_token_conversion::SubjectFor;

use crate::{
	AcurastHyperdriveIbc, Balances, OutgoingTransferTTL, Runtime, RuntimeEvent, RuntimeFreezeReason,
};

parameter_types! {
	pub const TokenConversionPalletId: PalletId = MainnetTokenConversionPalletId::get();
	pub TokenConversionPalletAccountId: AccountId = TokenConversionPalletId::get().into_account_truncating();
	pub const Chain: ProxyChain = ProxyChain::Acurast;
	pub ReceiveFrom: Option<SubjectFor<Runtime>> = Some(Subject::AcurastCanary(Layer::Extrinsic(CanaryTokenConversionPalletId::get().into_account_truncating())));
	pub const SendTo: Option<SubjectFor<Runtime>> = None;
	pub const Liquidity: Balance = UNIT;
	pub const MaxLockDuration: BlockNumber = 48 * 28 * DAYS;
}

impl pallet_acurast_token_conversion::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type PalletId = TokenConversionPalletId;
	type Chain = Chain;
	type SendTo = SendTo;
	type ReceiveFrom = ReceiveFrom;
	type Currency = Balances;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type Liquidity = Liquidity;
	type MaxLockDuration = MaxLockDuration;
	type MessageSender = AcurastHyperdriveIbc;
	type MessageIdHasher = BlakeTwo256;
	type OnSlash = ResolveTo<TokenConversionPalletAccountId, Balances>;
	type ConvertTTL = OutgoingTransferTTL;
	type EnableOrigin = EnsureRoot<Self::AccountId>;
	type WeightInfo = crate::weights::pallet_acurast_token_conversion::WeightInfo<Self>;
}
