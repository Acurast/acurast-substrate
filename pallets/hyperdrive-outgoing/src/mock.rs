use frame_support::weights::Weight;
use frame_support::{
    parameter_types, traits::ConstU32, weights::constants::RocksDbWeight as DbWeight,
};
use pallet_acurast_hyperdrive::instances::TezosInstance;
use sp_core::H256;
use sp_runtime::traits::AccountIdLookup;
use sp_runtime::traits::BlakeTwo256;

use stub::*;

use crate::chain::tezos::DefaultTezosConfig;
use crate::*;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>} = 0,
        HyperdriveOutgoing: crate::{Pallet, Call, Storage, Event<T>},
    }
);

impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Nonce = u64;
    type Block = Block<Test>;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = AccountIdLookup<AccountId, ()>;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = ();
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
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
