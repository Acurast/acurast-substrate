use frame_support::{parameter_types, PalletId};
use polkadot_core_primitives::BlakeTwo256;
use sp_runtime::traits::AccountIdConversion;

use acurast_runtime_common::{
	constants::{CanaryTokenConversionPalletId, MainnetTokenConversionPalletId, DAYS, UNIT},
	types::{Balance, BlockNumber},
};
use pallet_acurast::{Layer, Subject};
use pallet_acurast_token_conversion::SubjectFor;

use crate::{
	AcurastHyperdriveIbc, Balances, EnsureCouncilOrRoot, OutgoingTransferTTL, Runtime,
	RuntimeEvent, RuntimeHoldReason,
};

parameter_types! {
	pub const TokenConversionPalletId: PalletId = CanaryTokenConversionPalletId::get();
	pub SendTo: Option<SubjectFor<Runtime>> = Some(Subject::Acurast(Layer::Extrinsic(MainnetTokenConversionPalletId::get().into_account_truncating())));
	pub const ReceiveFrom: Option<SubjectFor<Runtime>> = None;
	pub const Liquidity: Balance = UNIT;
	pub const MinLockDuration: BlockNumber = 3 * 28 * DAYS;
	pub const MaxLockDuration: BlockNumber = 48 * 28 * DAYS;
}

impl pallet_acurast_token_conversion::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type PalletId = TokenConversionPalletId;
	type SendTo = SendTo;
	type ReceiveFrom = ReceiveFrom;
	type Currency = Balances;
	type RuntimeHoldReason = RuntimeHoldReason;
	type Liquidity = Liquidity;
	type MinLockDuration = MinLockDuration;
	type MaxLockDuration = MaxLockDuration;
	type MessageSender = AcurastHyperdriveIbc;
	type MessageIdHasher = BlakeTwo256;
	type OnSlash = ();
	type ConvertTTL = OutgoingTransferTTL;
	type EnableOrigin = EnsureCouncilOrRoot;
	type WeightInfo = crate::weights::pallet_acurast_token_conversion::WeightInfo<Self>;
}
