use acurast_runtime_common::{
	types::{AccountId, Balance, Hash, Nonce},
	weights::RocksDbWeight,
};
use frame_support::derive_impl;
use polkadot_runtime_common::BlockHashCount;

use crate::{
	Block, PalletInfo, Runtime, RuntimeBlockLength, RuntimeBlockWeights, RuntimeCall, RuntimeEvent,
	RuntimeOrigin, RuntimeTask, SS58Prefix, Version,
};

// Configure FRAME pallets to include in runtime.
#[derive_impl(frame_system::config_preludes::ParaChainDefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountId = AccountId;
	type Nonce = Nonce;
	type Hash = Hash;
	type Block = Block;
	type BlockHashCount = BlockHashCount;
	type Version = Version;
	type AccountData = pallet_balances::AccountData<Balance>;
	type DbWeight = RocksDbWeight;
	type BlockWeights = RuntimeBlockWeights;
	type BlockLength = RuntimeBlockLength;
	type SS58Prefix = SS58Prefix;
	type OnSetCode = cumulus_pallet_parachain_system::ParachainSetCode<Self>;
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type ExtensionsWeightInfo =
		acurast_runtime_common::weight::frame_system_extensions::WeightInfo<Self>;
}

/// Runtime configuration for parachain_info.
impl parachain_info::Config for Runtime {}
