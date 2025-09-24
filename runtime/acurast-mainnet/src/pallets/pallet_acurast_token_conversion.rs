use frame_support::{parameter_types, PalletId};
use frame_system::EnsureRoot;
use polkadot_core_primitives::BlakeTwo256;
use sp_runtime::traits::AccountIdConversion;

use acurast_runtime_common::{
	constants::{CanaryTokenConversionPalletId, MainnetTokenConversionPalletId, DAYS, UNIT},
	types::{Balance, BlockNumber},
};
use pallet_acurast::ProxyChain;
use pallet_acurast_hyperdrive_ibc::{Subject, SubjectFor};

use crate::{
	AcurastHyperdriveIbc, Balances, OutgoingTransferTTL, Runtime, RuntimeEvent, RuntimeFreezeReason,
};

parameter_types! {
	pub const TokenConversionPalletId: PalletId = MainnetTokenConversionPalletId::get();
	pub const Chain: ProxyChain = ProxyChain::Acurast;
	pub ReceiveFrom: Option<SubjectFor<Runtime>> = Some(Subject::Acurast(CanaryTokenConversionPalletId::get().into_account_truncating()));
	pub const SendTo: Option<SubjectFor<Runtime>> = None;
	pub const Liquidity: Balance = UNIT / 100;
	pub const MinLockDuration: BlockNumber = 3 * 28 * DAYS;
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
	type MinLockDuration = MinLockDuration;
	type MaxLockDuration = MaxLockDuration;
	type MessageSender = AcurastHyperdriveIbc;
	type MessageIdHasher = BlakeTwo256;
	type ConvertTTL = OutgoingTransferTTL;
	type Hook = ();
	type EnableOrigin = EnsureRoot<Self::AccountId>;
	type WeightInfo = pallet_acurast_token_conversion::weights::WeightInfo<Self>;
}
