use crate::{chain::tezos::DefaultTezosConfig, *};
use frame_support::{
	derive_impl, parameter_types,
	sp_runtime::traits::{ConstU16, ConstU32, ConstU64, IdentityLookup},
	weights::{constants::RocksDbWeight as DbWeight, Weight},
};
use pallet_acurast_hyperdrive::instances::TezosInstance;
use sp_core::H256;
use stub::*;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>} = 0,
		HyperdriveOutgoing: crate::{Pallet, Call, Storage, Event<T>},
	}
);

#[derive_impl(frame_system::config_preludes::ParaChainDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Nonce = u64;
	type Hash = H256;
	type Block = Block<Test>;
	type BlockHashCount = ConstU64<250>;
	type Version = ();
	type AccountData = ();
	type DbWeight = ();
	type BlockWeights = ();
	type BlockLength = ();
	type SS58Prefix = ConstU16<42>;
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MMRInfo = TezosInstance;
	type TargetChainConfig = DefaultTezosConfig;
	type OnNewRoot = ();
	type WeightInfo = ();
	type MaximumBlocksBeforeSnapshot = MaximumBlocksBeforeSnapshot;
}

impl WeightInfo for () {
	fn send_message() -> Weight {
		DbWeight::get().reads_writes(3, 3)
	}
}

parameter_types! {
	pub const BlockHashCount: BlockNumber = 2400;

	pub const MaximumBlocksBeforeSnapshot: u64 = 2;
}
