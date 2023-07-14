use crate::{AcurastAsset, Extra, MultiSignature, RegistrationExtra};
use pallet_acurast_marketplace::JobRequirements;
use sp_core::crypto::UncheckedFrom;
use sp_runtime::AccountId32 as AccountId;
use xcm::{
	latest::prelude::{X2, X3},
	prelude::{Concrete, Fungible, GeneralIndex, PalletInstance, Parachain},
	v2::{MultiAsset, MultiLocation},
};

pub struct AcurastBenchmarkHelper;

impl pallet_acurast::benchmarking::BenchmarkHelper<crate::Runtime> for AcurastBenchmarkHelper {
	fn registration_extra() -> Extra {
		Extra {
			requirements: JobRequirements {
				slots: 1,
				reward: 20100,
				min_reputation: None,
				instant_match: None,
			},
		}
	}

	fn funded_account(index: u32) -> <crate::Runtime as frame_system::Config>::AccountId {
		[(index + 1) as u8; 32].into()
	}
}

impl pallet_acurast_marketplace::benchmarking::BenchmarkHelper<crate::Runtime>
	for AcurastBenchmarkHelper
{
	fn registration_extra(
		r: pallet_acurast_marketplace::JobRequirementsFor<crate::Runtime>,
	) -> <crate::Runtime as pallet_acurast_marketplace::Config>::RegistrationExtra {
		Extra { requirements: r }
	}

	fn funded_account(
		index: u32,
		_amount: <crate::Runtime as pallet_acurast_marketplace::Config>::Balance,
	) -> <crate::Runtime as frame_system::Config>::AccountId {
		[(index + 1) as u8; 32].into()
	}
}

impl pallet_acurast_processor_manager::benchmarking::BenchmarkHelper<crate::Runtime>
	for AcurastBenchmarkHelper
{
	fn dummy_proof() -> <crate::Runtime as pallet_acurast_processor_manager::Config>::Proof {
		MultiSignature::Sr25519(sp_core::sr25519::Signature::unchecked_from([0u8; 64]))
	}
}
