use crate::{AcurastAsset, RegistrationExtra};
use pallet_acurast_marketplace::JobRequirements;
use xcm::{
	latest::prelude::{X2, X3},
	prelude::{Concrete, Fungible, GeneralIndex, PalletInstance, Parachain},
	v2::{MultiAsset, MultiLocation},
};

impl Default for RegistrationExtra {
	fn default() -> Self {
		RegistrationExtra {
			destination: MultiLocation {
				parents: 1,
				interior: X2(Parachain(2001), PalletInstance(40)),
			},
			parameters: None,
			requirements: JobRequirements {
				slots: 1,
				cpu_milliseconds: 5,
				reward: AcurastAsset(MultiAsset {
					id: Concrete(MultiLocation {
						parents: 1,
						interior: X3(Parachain(1000), PalletInstance(50), GeneralIndex(22)),
					}),
					fun: Fungible(20100),
				}),
			},
		}
	}
}
