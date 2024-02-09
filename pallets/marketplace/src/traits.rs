use frame_support::{pallet_prelude::DispatchError, weights::Weight};

/// Trait used to lookup the manager of a given processor account.
pub trait ManagerProvider<T: frame_system::Config> {
    fn manager_of(owner: &T::AccountId) -> Result<T::AccountId, DispatchError>;
}

/// Trait used to lookup the time a processor was last seen, i.e. sent a heartbeat.
pub trait ProcessorLastSeenProvider<T: frame_system::Config> {
    fn last_seen(processor: &T::AccountId) -> Option<u128>;
}

/// Weight functions needed for pallet_acurast_marketplace.
pub trait WeightInfo {
    fn advertise() -> Weight;
    fn delete_advertisement() -> Weight;
    fn report() -> Weight;
    fn propose_matching(x: u32) -> Weight;
    fn acknowledge_match() -> Weight;
    fn finalize_job() -> Weight;
    fn finalize_jobs(x: u32) -> Weight;
}
