use crate::{AcurastAsset, RegistrationExtra};
use pallet_acurast_marketplace::JobRequirements;
use xcm::prelude::{Concrete, Fungible, GeneralIndex, PalletInstance, Parachain};
use xcm::{
	latest::prelude::{X2, X3},
	v2::{MultiAsset, MultiLocation},
};

impl From<pallet_acurast_marketplace::benchmarking::MockAsset> for AcurastAsset {
	fn from(asset: pallet_acurast_marketplace::benchmarking::MockAsset) -> Self {
		AcurastAsset(MultiAsset {
			id: Concrete(MultiLocation {
				parents: 1,
				interior: X3(Parachain(1000), PalletInstance(50), GeneralIndex(asset.id as u128)),
			}),
			fun: Fungible(asset.amount),
		})
	}
}

impl From<JobRequirements<AcurastAsset>> for RegistrationExtra {
	fn from(requirements: JobRequirements<AcurastAsset>) -> Self {
		RegistrationExtra {
			destination: MultiLocation {
				parents: 1,
				interior: X2(Parachain(2001), PalletInstance(40)),
			},
			parameters: None,
			requirements,
		}
	}
}

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
