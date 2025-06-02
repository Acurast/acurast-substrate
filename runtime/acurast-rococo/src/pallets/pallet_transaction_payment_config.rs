use acurast_runtime_common::types::{Balance, TransactionCharger};
use frame_support::weights::ConstantMultiplier;
use polkadot_runtime_common::SlowAdjustingFeeUpdate;

use crate::{
	Balances, OperationalFeeMultiplier, ProcessorPairingProvider, Runtime, RuntimeEvent,
	TransactionByteFee, WeightToFee,
};

/// Runtime configuration for pallet_transaction_payment.
impl pallet_transaction_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction = TransactionCharger<Balances, (), ProcessorPairingProvider>;
	type WeightToFee = WeightToFee;
	type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
	type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Self>;
	type OperationalFeeMultiplier = OperationalFeeMultiplier;
	type WeightInfo = ();
}
