use acurast_runtime_common::{weights::RocksDbWeight, AccountId, Balance, Hash, Nonce};
use frame_support::derive_impl;
use polkadot_runtime_common::BlockHashCount;

use crate::{
	Block, CallFilter, PalletInfo, Runtime, RuntimeBlockLength, RuntimeBlockWeights, RuntimeCall,
	RuntimeEvent, RuntimeOrigin, RuntimeTask, SS58Prefix, Version,
};

#[derive_impl(frame_system::config_preludes::ParaChainDefaultConfig as frame_system::DefaultConfig)]
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
	type BaseCallFilter = CallFilter;
	type SS58Prefix = SS58Prefix;
	type OnSetCode = cumulus_pallet_parachain_system::ParachainSetCode<Self>;
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

/// Runtime configuration for parachain_info.
impl parachain_info::Config for Runtime {}
