use frame_support::weights::ConstantMultiplier;
use polkadot_runtime_common::SlowAdjustingFeeUpdate;

use acurast_runtime_common::types::{Balance, IsFundable, TransactionCharger};

use crate::{
	AcurastMarketplace, AcurastProcessorManager, Balances, OperationalFeeMultiplier, Runtime,
	RuntimeEvent, TransactionByteFee, WeightToFee,
};

/// Runtime configuration for pallet_transaction_payment.
impl pallet_transaction_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction = TransactionCharger<
		Balances,
		(),
		IsFundable<Self, AcurastProcessorManager, AcurastMarketplace>,
		AcurastProcessorManager,
	>;
	type WeightToFee = WeightToFee;
	type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
	type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Self>;
	type OperationalFeeMultiplier = OperationalFeeMultiplier;
	type WeightInfo = ();
}
