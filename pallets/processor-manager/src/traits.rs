use frame_support::pallet_prelude::Weight;
use frame_support::{pallet_prelude::DispatchResult, sp_runtime::DispatchError};

use crate::Config;

pub trait ManagerIdProvider<T: Config> {
    fn create_manager_id(id: T::ManagerId, owner: &T::AccountId) -> DispatchResult;
    fn manager_id_for(owner: &T::AccountId) -> Result<T::ManagerId, DispatchError>;
    fn owner_for(manager_id: T::ManagerId) -> Result<T::AccountId, DispatchError>;
}

pub trait ProcessorAssetRecovery<T: Config> {
    fn recover_assets(
        processor: &T::AccountId,
        destination_account: &T::AccountId,
    ) -> DispatchResult;
}

pub trait AdvertisementHandler<T: Config> {
    fn advertise_for(processor: &T::AccountId, advertisement: &T::Advertisement) -> DispatchResult;
}

impl<T: Config> AdvertisementHandler<T> for () {
    fn advertise_for(
        _processor: &T::AccountId,
        _advertisement: &T::Advertisement,
    ) -> DispatchResult {
        Ok(())
    }
}

/// Weight functions needed for pallet_acurast_processor_manager.
pub trait WeightInfo {
    fn update_processor_pairings(x: u32) -> Weight;
    fn pair_with_manager() -> Weight;
    fn recover_funds() -> Weight;
    fn heartbeat() -> Weight;
    fn heartbeat_with_version() -> Weight;
    fn advertise_for() -> Weight;
    fn update_binary_hash() -> Weight;
    fn set_processor_update_info(x: u32) -> Weight;
}
