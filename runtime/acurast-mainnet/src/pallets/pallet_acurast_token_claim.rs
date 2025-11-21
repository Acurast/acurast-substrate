use frame_support::parameter_types;
use sp_runtime::traits::ConvertInto;

use acurast_runtime_common::{
	constants::DAYS,
	types::{AccountId, BlockNumber, Signature},
	weight,
};

#[cfg(feature = "runtime-benchmarks")]
use crate::benchmarking::AcurastBenchmarkHelper;
use crate::{Runtime, RuntimeEvent, Vesting};

const MONTH: BlockNumber = 30 * DAYS;

parameter_types! {
	pub Signer: AccountId = AccountId::from(hex_literal::hex!("546bfb7826b72e20d41d7ece2ab05b11440906622f4c16042816604d663a465b"));
	pub Funder: AccountId = AccountId::from(hex_literal::hex!("747f83e907bfecee52377542fc2f65181f224d59dcfd69eef059b1c3203f1501"));
	pub VestingDuration: BlockNumber = 24 * MONTH;
}

impl pallet_acurast_token_claim::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type VestedTransferer = Vesting;
	type Signature = Signature;
	type Signer = Signer;
	type Funder = Funder;
	type VestingDuration = VestingDuration;
	type BlockNumberToBalance = ConvertInto;
	type WeightInfo = weight::pallet_acurast_token_claim::WeightInfo<Self>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = AcurastBenchmarkHelper;
}
