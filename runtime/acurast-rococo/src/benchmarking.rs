use crate::{AcurastAsset, AcurastAssetId, Extra, RegistrationExtra};
use pallet_acurast_marketplace::{JobRequirements, MultiDestination};
use sp_runtime::AccountId32 as AccountId;
use xcm::{
	latest::prelude::{X2, X3},
	prelude::{Concrete, Fungible, GeneralIndex, PalletInstance, Parachain},
	v2::{MultiAsset, MultiLocation},
};

pub struct AcurastBenchmarkHelper;

impl pallet_assets::BenchmarkHelper<codec::Compact<AcurastAssetId>> for AcurastBenchmarkHelper {
	fn create_asset_id_parameter(id: u32) -> codec::Compact<AcurastAssetId> {
		let asset_id = AssetId::Concrete(MultiLocation::new(
			1,
			X3(Parachain(1000), PalletInstance(50), GeneralIndex(id as u128)),
		));
		codec::Compact(asset_id)
	}
}

impl pallet_acurast_assets_manager::benchmarking::BenchmarkHelper<codec::Compact<AcurastAssetId>>
	for AcurastBenchmarkHelper
{
	fn manager_account() -> frame_system::pallet::AccountId {
		[0; 32].into()
	}
}

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

impl pallet_acurast::benchmarking::BenchmarkHelper<crate::Runtime> for AcurastBenchmarkHelper {
	fn registration_extra() -> Extra {
		Extra {
			destination: MultiDestination::Acurast(MultiLocation {
				parents: 1,
				interior: X2(Parachain(2001), PalletInstance(40)),
			}),
			parameters: None,
			requirements: JobRequirements {
				slots: 1,
				reward: AcurastAsset(MultiAsset {
					id: Concrete(MultiLocation {
						parents: 1,
						interior: X3(Parachain(1000), PalletInstance(50), GeneralIndex(22)),
					}),
					fun: Fungible(20100),
				}),
				min_reputation: None,
				instant_match: None,
			},
			expected_fulfillment_fee: 200,
		}
	}
}
