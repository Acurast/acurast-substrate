use core::marker::PhantomData;

use acurast_runtime_common::{weights, AccountId};
use frame_support::{instances::Instance1, pallet_prelude::DispatchResultWithPostInfo};
use pallet_acurast_hyperdrive_ibc::{LayerFor, MessageBody, SubjectFor};
use polkadot_core_primitives::BlakeTwo256;

use crate::{
	AcurastHyperdrive, AcurastPalletAccount, Balances, MinDeliveryConfirmationSignatures,
	MinReceiptConfirmationSignatures, MinTTL, Runtime, RuntimeEvent, RuntimeHoldReason,
};

impl pallet_acurast_hyperdrive_ibc::Config<Instance1> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MinTTL = MinTTL;
	type MinDeliveryConfirmationSignatures = MinDeliveryConfirmationSignatures;
	type MinReceiptConfirmationSignatures = MinReceiptConfirmationSignatures;
	type Currency = Balances;
	type RuntimeHoldReason = RuntimeHoldReason;
	type MessageIdHashing = BlakeTwo256;
	type MessageProcessor = HyperdriveMessageProcessor<Runtime>;
	type WeightInfo = weights::HyperdriveWeight;
}

/// Controls routing for incoming HyperdriveIBC messages.
///
/// Currently only forwards messages with recipient [`AcurastPalletAccount`] to AcurastHyperdrive pallet.
pub struct HyperdriveMessageProcessor<T: pallet_acurast::Config>(PhantomData<T>);
impl pallet_acurast_hyperdrive_ibc::MessageProcessor<AccountId, AccountId>
	for HyperdriveMessageProcessor<Runtime>
{
	fn process(message: MessageBody<AccountId, AccountId>) -> DispatchResultWithPostInfo {
		if SubjectFor::<Runtime>::Acurast(LayerFor::<Runtime>::Extrinsic(
			AcurastPalletAccount::get(),
		)) == message.recipient
		{
			AcurastHyperdrive::process(message)
		} else {
			// TODO fail this?
			Ok(().into())
		}
	}
}
