use core::marker::PhantomData;
use frame_support::{
	instances::Instance1, pallet_prelude::DispatchResultWithPostInfo, parameter_types,
};
use frame_system::EnsureRoot;
use polkadot_core_primitives::BlakeTwo256;

use acurast_runtime_common::{
	constants::UNIT,
	types::{AccountId, Balance},
	weight,
};
use pallet_acurast::{MessageBody, MessageProcessor};
use pallet_acurast_hyperdrive::ParsedAction;
use pallet_acurast_hyperdrive_ibc::{LayerFor, SubjectFor};

use crate::{
	Acurast, AcurastAccountId, AcurastHyperdrive, AcurastHyperdriveIbc, AcurastHyperdriveToken,
	AcurastPalletAccount, AlephZeroContract, AlephZeroContractSelector, Balances,
	HyperdriveTokenEthereumFeeVault, HyperdriveTokenEthereumVault, HyperdriveTokenPalletAccount,
	HyperdriveTokenSolanaFeeVault, HyperdriveTokenSolanaVault, IncomingTTL,
	MinDeliveryConfirmationSignatures, MinReceiptConfirmationSignatures, MinTTL,
	OperationalFeeAccount, OutgoingTransferTTL, ParachainInfo, Runtime, RuntimeEvent,
	RuntimeHoldReason, VaraContract,
};

parameter_types! {
	pub const MinFee: Balance = UNIT / 10;
	pub const MinTransferAmount: Balance = UNIT;
}

impl pallet_acurast_hyperdrive::Config<Instance1> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ActionExecutor = AcurastActionExecutor<Runtime>;
	type Sender = AcurastPalletAccount;
	type ParsableAccountId = AcurastAccountId;
	type AlephZeroContract = AlephZeroContract;
	type AlephZeroContractSelector = AlephZeroContractSelector;
	type VaraContract = VaraContract;
	type Balance = Balance;
	type MessageSender = AcurastHyperdriveIbc;
	type MessageIdHasher = BlakeTwo256;
	type UpdateOrigin = EnsureRoot<Self::AccountId>;
	type WeightInfo = weight::pallet_acurast_hyperdrive::WeightInfo<Runtime>;
}

impl pallet_acurast_hyperdrive_ibc::Config<Instance1> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MinTTL = MinTTL;
	type IncomingTTL = IncomingTTL;
	type MinDeliveryConfirmationSignatures = MinDeliveryConfirmationSignatures;
	type MinReceiptConfirmationSignatures = MinReceiptConfirmationSignatures;
	type MinFee = MinFee;
	type Currency = Balances;
	type RuntimeHoldReason = RuntimeHoldReason;
	type MessageIdHashing = BlakeTwo256;
	type MessageProcessor = HyperdriveMessageProcessor;
	type UpdateOrigin = EnsureRoot<Self::AccountId>;
	type ParachainId = ParachainInfo;
	type WeightInfo = weight::pallet_acurast_hyperdrive_ibc::WeightInfo<Self>;
}

impl pallet_acurast_hyperdrive_token::Config<Instance1> for Runtime {
	type RuntimeEvent = RuntimeEvent;

	type PalletAccount = HyperdriveTokenPalletAccount;
	type ParsableAccountId = AcurastAccountId;
	type Balance = Balance;
	type Currency = Balances;
	type MessageSender = AcurastHyperdriveIbc;
	type MessageIdHasher = BlakeTwo256;

	type EthereumVault = HyperdriveTokenEthereumVault;
	type EthereumFeeVault = HyperdriveTokenEthereumFeeVault;
	type SolanaVault = HyperdriveTokenSolanaVault;
	type SolanaFeeVault = HyperdriveTokenSolanaFeeVault;
	type OperationalFeeAccount = OperationalFeeAccount;
	type OutgoingTransferTTL = OutgoingTransferTTL;
	type UpdateOrigin = EnsureRoot<Self::AccountId>;
	type OperatorOrigin = EnsureRoot<Self::AccountId>;
	type MinTransferAmount = MinTransferAmount;

	type WeightInfo = weight::pallet_acurast_hyperdrive_token::WeightInfo<Runtime>;
}

pub struct AcurastActionExecutor<T: pallet_acurast::Config>(PhantomData<T>);
impl pallet_acurast_hyperdrive::ActionExecutor<Runtime> for AcurastActionExecutor<Runtime> {
	fn execute(action: ParsedAction<Runtime>) -> DispatchResultWithPostInfo {
		match action {
			ParsedAction::RegisterJob(job_id, registration) => {
				Acurast::register_for(job_id, registration, None)
			},
			ParsedAction::DeregisterJob(job_id) => Acurast::deregister_for(job_id),
			ParsedAction::FinalizeJob(_job_ids) => {
				log::warn!("FinalizedJob is deprecated, just use DeregisterJob instead");

				Ok(().into())
			},
			ParsedAction::SetJobEnvironment(job_id, environments) => {
				Acurast::set_environment_for(job_id, environments)?;
				Ok(().into())
			},
			ParsedAction::Noop => {
				// Intentionally, just logging it
				log::debug!("Received NOOP operation from hyperdrive");

				Ok(().into())
			},
		}
	}
}

/// Controls routing for incoming HyperdriveIBC messages.
///
/// Forwards messages with
/// * recipient [`AcurastPalletAccount`] to AcurastHyperdrive pallet,
/// * recipient [`HyperdriveTokenPalletAccount`] to AcurastHyperdriveToken pallet.
pub struct HyperdriveMessageProcessor;
impl MessageProcessor<AccountId, AccountId> for HyperdriveMessageProcessor {
	fn process(message: impl MessageBody<AccountId, AccountId>) -> DispatchResultWithPostInfo {
		if &SubjectFor::<Runtime>::Acurast(LayerFor::<Runtime>::Extrinsic(
			AcurastPalletAccount::get(),
		)) == message.recipient()
		{
			AcurastHyperdrive::process(message)
		} else if &SubjectFor::<Runtime>::Acurast(LayerFor::<Runtime>::Extrinsic(
			HyperdriveTokenPalletAccount::get(),
		)) == message.recipient()
		{
			AcurastHyperdriveToken::process(message)
		} else {
			// TODO fail this?
			Ok(().into())
		}
	}
}
