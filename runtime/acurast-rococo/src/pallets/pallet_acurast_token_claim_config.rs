use frame_support::parameter_types;
use sp_runtime::traits::ConvertInto;

use acurast_runtime_common::{
	constants::MINUTES,
	types::{AccountId, BlockNumber, Signature},
	weight,
};

#[cfg(feature = "runtime-benchmarks")]
use crate::benchmarking::AcurastBenchmarkHelper;
use crate::{Balances, Runtime, RuntimeEvent};

parameter_types! {
	// Testnet signer - using a well-known test account
	pub Signer: AccountId = AccountId::from(hex_literal::hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d")); // Alice
	// Testnet funder - using a well-known test account
	pub Funder: AccountId = AccountId::from(hex_literal::hex!("306721211d5404bd9da88e0204360a1a9ab8b87c66c1bc2fcdd37f3c2222cc20")); // Dave
	pub VestingDuration: BlockNumber = 5 * MINUTES;
}

impl pallet_acurast_token_claim::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type Signature = Signature;
	type Signer = Signer;
	type Funder = Funder;
	type VestingDuration = VestingDuration;
	type BlockNumberToBalance = ConvertInto;
	type WeightInfo = weight::pallet_acurast_token_claim::WeightInfo<Self>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = AcurastBenchmarkHelper;
}
