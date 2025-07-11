use core::marker::PhantomData;

use acurast_runtime_common::{
	types::{AccountId, Balance},
	weight, weights,
};
use frame_support::{instances::Instance1, pallet_prelude::DispatchResultWithPostInfo};
use pallet_acurast_hyperdrive::ParsedAction;
use pallet_acurast_hyperdrive_ibc::{LayerFor, MessageBody, SubjectFor};
use polkadot_core_primitives::BlakeTwo256;

use crate::{
	Acurast, AcurastAccountId, AcurastHyperdrive, AcurastHyperdriveToken, AcurastMarketplace,
	AcurastPalletAccount, AlephZeroContract, AlephZeroContractSelector, Balances,
	HyperdriveTokenEthereumFeeVault, HyperdriveTokenEthereumVault, HyperdriveTokenPalletAccount,
	HyperdriveTokenSolanaFeeVault, HyperdriveTokenSolanaVault, IncomingTTL,
	MinDeliveryConfirmationSignatures, MinReceiptConfirmationSignatures, MinTTL,
	OutgoingTransferTTL, Runtime, RuntimeEvent, RuntimeHoldReason, VaraContract,
};

impl pallet_acurast_hyperdrive::Config<Instance1> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ActionExecutor = AcurastActionExecutor<Runtime>;
	type Sender = AcurastPalletAccount;
	type ParsableAccountId = AcurastAccountId;
	type AlephZeroContract = AlephZeroContract;
	type AlephZeroContractSelector = AlephZeroContractSelector;
	type VaraContract = VaraContract;
	type Balance = Balance;
	type WeightInfo = weight::pallet_acurast_hyperdrive::WeightInfo<Runtime>;
}

impl pallet_acurast_hyperdrive_ibc::Config<Instance1> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MinTTL = MinTTL;
	type IncomingTTL = IncomingTTL;
	type MinDeliveryConfirmationSignatures = MinDeliveryConfirmationSignatures;
	type MinReceiptConfirmationSignatures = MinReceiptConfirmationSignatures;
	type Currency = Balances;
	type RuntimeHoldReason = RuntimeHoldReason;
	type MessageIdHashing = BlakeTwo256;
	type MessageProcessor = HyperdriveMessageProcessor;
	type WeightInfo = weights::HyperdriveWeight;
}

impl pallet_acurast_hyperdrive_token::Config<Instance1> for Runtime {
	type RuntimeEvent = RuntimeEvent;

	type PalletAccount = HyperdriveTokenPalletAccount;
	type ParsableAccountId = AcurastAccountId;
	type Balance = Balance;

	type EthereumVault = HyperdriveTokenEthereumVault;
	type EthereumFeeVault = HyperdriveTokenEthereumFeeVault;
	type SolanaVault = HyperdriveTokenSolanaVault;
	type SolanaFeeVault = HyperdriveTokenSolanaFeeVault;
	type OutgoingTransferTTL = OutgoingTransferTTL;

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
			ParsedAction::FinalizeJob(job_ids) => AcurastMarketplace::finalize_jobs_for(job_ids),
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
impl pallet_acurast_hyperdrive_ibc::MessageProcessor<AccountId, AccountId>
	for HyperdriveMessageProcessor
{
	fn process(message: MessageBody<AccountId, AccountId>) -> DispatchResultWithPostInfo {
		if SubjectFor::<Runtime>::Acurast(LayerFor::<Runtime>::Extrinsic(
			AcurastPalletAccount::get(),
		)) == message.recipient
		{
			AcurastHyperdrive::process(message)
		} else if SubjectFor::<Runtime>::Acurast(LayerFor::<Runtime>::Extrinsic(
			HyperdriveTokenPalletAccount::get(),
		)) == message.recipient
		{
			AcurastHyperdriveToken::process(message)
		} else {
			// TODO fail this?
			Ok(().into())
		}
	}
}
