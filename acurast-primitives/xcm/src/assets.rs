

use sp_std::borrow::Borrow;
use frame_support::weights::Weight;
use sp_runtime::traits::Bounded;
use xcm::v2::{MultiLocation};


pub struct IdStoreRetriever<AssetId, AssetMapper>(
    sp_std::marker::PhantomData<(AssetId, AssetMapper)>,
);
impl<AssetId, AssetMapper> xcm_executor::traits::Convert<MultiLocation, AssetId>
for IdStoreRetriever<AssetId, AssetMapper>
    where
        AssetId: Clone + Eq + Bounded,
        AssetMapper: StorageRetriever<AssetId>,
{
    fn convert_ref(location: impl Borrow<MultiLocation>) -> Result<AssetId, ()> {
        if let Some(asset_id) = AssetMapper::get_asset_id(location.borrow().clone()) {
            Ok(asset_id)
        } else {
            Err(())
        }
    }

    fn reverse_ref(id: impl Borrow<AssetId>) -> Result<MultiLocation, ()> {
        if let Some(multilocation) = AssetMapper::get_asset_location(id.borrow().clone()) {
            Ok(multilocation)
        } else {
            Err(())
        }
    }
}

pub trait StorageRetriever<AssetId> {
    /// Get asset type from assetId
    fn get_asset_location(asset_id: AssetId) -> Option<MultiLocation>;

    /// Get local asset Id from asset location
    fn get_asset_id(asset_location: MultiLocation) -> Option<AssetId>;
}

/// Weight functions needed for pallet_xcm_assets.
pub trait WeightInfo {
    fn register_asset_location() -> Weight;
    fn set_asset_units_per_second() -> Weight;
    fn change_existing_asset_location() -> Weight;
    fn remove_payment_asset() -> Weight;
    fn remove_asset() -> Weight;
}
