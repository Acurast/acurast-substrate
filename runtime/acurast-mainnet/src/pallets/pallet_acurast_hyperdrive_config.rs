use frame_support::{
	instances::Instance1, pallet_prelude::DispatchResultWithPostInfo, parameter_types,
};
use polkadot_core_primitives::BlakeTwo256;

use acurast_runtime_common::{
	constants::UNIT,
	types::{AccountId, Balance},
	weight,
};
use pallet_acurast::{MessageBody, MessageProcessor, ProxyAcurastChain};
use pallet_acurast_hyperdrive_ibc::{LayerFor, SubjectFor};

use crate::{
	AcurastAccountId, AcurastHyperdriveIbc, AcurastHyperdriveToken, Balances, EnsureCouncilOrRoot,
	HyperdriveTokenEthereumFeeVault, HyperdriveTokenEthereumVault, HyperdriveTokenPalletAccount,
	HyperdriveTokenSolanaFeeVault, HyperdriveTokenSolanaVault, IncomingTTL,
	MinDeliveryConfirmationSignatures, MinReceiptConfirmationSignatures, MinTTL,
	OperationalFeeAccount, OutgoingTransferTTL, ParachainInfo, Runtime, RuntimeHoldReason,
};

parameter_types! {
	pub const MinFee: Balance = UNIT * 20;
	pub const MinTransferAmount: Balance = UNIT;
	pub const SelfChain: ProxyAcurastChain = ProxyAcurastChain::Acurast;
}

impl pallet_acurast_hyperdrive_ibc::Config<Instance1> for Runtime {
	type MinTTL = MinTTL;
	type IncomingTTL = IncomingTTL;
	type MinDeliveryConfirmationSignatures = MinDeliveryConfirmationSignatures;
	type MinReceiptConfirmationSignatures = MinReceiptConfirmationSignatures;
	type MinFee = MinFee;
	type Currency = Balances;
	type RuntimeHoldReason = RuntimeHoldReason;
	type MessageIdHashing = BlakeTwo256;
	type MessageProcessor = HyperdriveMessageProcessor;
	type UpdateOrigin = EnsureCouncilOrRoot;
	type ParachainId = ParachainInfo;
	type SelfChain = SelfChain;
	type WeightInfo = weight::pallet_acurast_hyperdrive_ibc::WeightInfo<Self>;
}

impl pallet_acurast_hyperdrive_token::Config<Instance1> for Runtime {
	type PalletAccount = HyperdriveTokenPalletAccount;
	type ParsableAccountId = AcurastAccountId;
	type Balance = Balance;
	type Currency = Balances;
	type MessageSender = AcurastHyperdriveIbc;
	type MessageIdHasher = BlakeTwo256;

	type EthereumVault = HyperdriveTokenEthereumVault;
	type EthereumFeeVault = HyperdriveTokenEthereumFeeVault;
	type OperationalFeeAccount = OperationalFeeAccount;
	type SolanaVault = HyperdriveTokenSolanaVault;
	type SolanaFeeVault = HyperdriveTokenSolanaFeeVault;
	type DefaultOutgoingTransferTTL = OutgoingTransferTTL;
	type UpdateOrigin = EnsureCouncilOrRoot;
	type OperatorOrigin = EnsureCouncilOrRoot;
	type MinTransferAmount = MinTransferAmount;

	type WeightInfo = weight::pallet_acurast_hyperdrive_token::WeightInfo<Runtime>;
}

/// Controls routing for incoming HyperdriveIBC messages.
///
/// Forwards messages with
/// * recipient [`HyperdriveTokenPalletAccount`] to AcurastHyperdriveToken pallet.
pub struct HyperdriveMessageProcessor;
impl MessageProcessor<AccountId, AccountId> for HyperdriveMessageProcessor {
	fn process(message: impl MessageBody<AccountId, AccountId>) -> DispatchResultWithPostInfo {
		if &SubjectFor::<Runtime>::Acurast(LayerFor::<Runtime>::Extrinsic(
			HyperdriveTokenPalletAccount::get(),
		)) == message.recipient()
		{
			AcurastHyperdriveToken::process(message)
		} else {
			// Unknown recipient (e.g. removed job-operation or token-conversion routes): no-op.
			Ok(().into())
		}
	}
}
