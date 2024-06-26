use frame_support::{
	instances::Instance1,
	pallet_prelude::{Decode, Encode},
};
use scale_info::TypeInfo;
use sp_core::RuntimeDebug;

#[derive(RuntimeDebug, Encode, Decode, TypeInfo)]
pub enum HyperdriveInstance {
	AcurastBidirectional,
}

/// Default instance that should be used to install Acurast sender/receiver interfaces.
pub type AcurastBidirectionalInstance = Instance1;

pub trait HyperdriveInstanceName {
	const NAME: HyperdriveInstance;
}

impl HyperdriveInstanceName for AcurastBidirectionalInstance {
	const NAME: HyperdriveInstance = HyperdriveInstance::AcurastBidirectional;
}
