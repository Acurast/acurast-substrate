use frame_support::instances::{Instance1, Instance2, Instance3};
use frame_support::pallet_prelude::{Decode, Encode};
use scale_info::TypeInfo;
use sp_core::RuntimeDebug;

#[derive(RuntimeDebug, Encode, Decode, TypeInfo)]
pub enum HyperdriveInstance {
    Tezos,
    Ethereum,
    AlephZero,
}

pub type TezosInstance = Instance1;
pub type EthereumInstance = Instance2;
pub type AlephZeroInstance = Instance3;

pub trait HyperdriveInstanceName {
    const NAME: HyperdriveInstance;
}

impl HyperdriveInstanceName for TezosInstance {
    const NAME: HyperdriveInstance = HyperdriveInstance::Tezos;
}

impl HyperdriveInstanceName for EthereumInstance {
    const NAME: HyperdriveInstance = HyperdriveInstance::Ethereum;
}

impl HyperdriveInstanceName for AlephZeroInstance {
    const NAME: HyperdriveInstance = HyperdriveInstance::AlephZero;
}
