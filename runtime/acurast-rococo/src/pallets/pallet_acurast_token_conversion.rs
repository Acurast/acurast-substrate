use frame_support::{parameter_types, traits::tokens::imbalance::ResolveTo, PalletId};
use frame_system::EnsureRoot;
use polkadot_core_primitives::BlakeTwo256;
use sp_runtime::{
	traits::{AccountIdConversion, Hash},
	DispatchError,
};
use sp_std::prelude::*;

use acurast_runtime_common::{
	constants::{MINUTES, UNIT},
	types::{AccountId, Balance, BlockNumber},
};
use pallet_acurast::ProxyChain;
use pallet_acurast_hyperdrive_ibc::{
	Layer, MessageFor, MessageProcessor, MessageSender, Subject, SubjectFor,
};

use crate::{
	AcurastTokenConversion, Balances, OutgoingTransferTTL, Runtime, RuntimeEvent,
	RuntimeFreezeReason,
};

parameter_types! {
	pub const TokenConversionPalletId: PalletId = PalletId(*b"tcdevpid");
	pub TokenConversionPalletAccountId: AccountId = TokenConversionPalletId::get().into_account_truncating();
	pub const Chain: ProxyChain = ProxyChain::Acurast;
	pub SendTo: Option<SubjectFor<Runtime>> = Some(Subject::Acurast(Layer::Extrinsic(TokenConversionPalletId::get().into_account_truncating())));
	pub ReceiveFrom: Option<SubjectFor<Runtime>> = Some(Subject::Acurast(Layer::Extrinsic(TokenConversionPalletId::get().into_account_truncating())));
	pub const Liquidity: Balance = UNIT / 100;
	pub const MaxLockDuration: BlockNumber = 5 * MINUTES;
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
	type MessageSender = LocalMessageSender;
	type MessageIdHasher = BlakeTwo256;
	type OnSlash = ResolveTo<TokenConversionPalletAccountId, Balances>;
	type ConvertTTL = OutgoingTransferTTL;
	type EnableOrigin = EnsureRoot<Self::AccountId>;
	type WeightInfo = pallet_acurast_token_conversion::weights::WeightInfo<Self>;
}

pub struct LocalMessageSender;
impl MessageSender<Runtime, Balance> for LocalMessageSender {
	fn send_message(
		sender: SubjectFor<Runtime>,
		_payer: &<Runtime as frame_system::Config>::AccountId,
		nonce: pallet_acurast_hyperdrive_ibc::MessageNonce,
		recipient: SubjectFor<Runtime>,
		payload: Vec<u8>,
		_ttl: frame_system::pallet_prelude::BlockNumberFor<Runtime>,
		_fee: Balance,
	) -> frame_support::dispatch::DispatchResult {
		let id = <Runtime as pallet_acurast_token_conversion::Config>::MessageIdHasher::hash_of(&(
			sender.clone(),
			nonce,
		));
		let message = MessageFor::<Runtime> {
			id,
			sender,
			nonce,
			recipient,
			payload: payload.try_into().map_err(|_| DispatchError::Other("Payload too long"))?,
		};
		_ = <AcurastTokenConversion as MessageProcessor<
			<Runtime as frame_system::Config>::AccountId,
			<Runtime as frame_system::Config>::AccountId,
		>>::process(message.into())
		.map_err(|_| DispatchError::Other("Cannot processor message"))?;
		Ok(())
	}
}
