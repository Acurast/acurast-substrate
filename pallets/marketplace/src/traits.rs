use crate::{Config, Error, PartialJobRegistrationForMarketplace};
use frame_support::{pallet_prelude::DispatchError, weights::Weight};
use pallet_acurast::JobRegistrationFor;

/// Trait used to lookup the manager of a given processor account.
pub trait ManagerProvider<T: frame_system::Config> {
	fn manager_of(owner: &T::AccountId) -> Result<T::AccountId, DispatchError>;
}

/// Trait used to lookup the time a processor was last seen, i.e. sent a heartbeat.
pub trait ProcessorInfoProvider<T: frame_system::Config + crate::Config> {
	fn last_seen(processor: &T::AccountId) -> Option<u128>;
	fn processor_version(processor: &T::AccountId) -> Option<T::ProcessorVersion>;
}

/// Manages each job's budget by reserving/unreserving rewards that are externally strored, e.g. on a pallet account in `pallet_balances`.
pub trait StorageTracker<T: Config> {
	/// Locks aka reduces available storage capacity by `registration`s required amount.
	fn check(
		source: &T::AccountId,
		registration: &PartialJobRegistrationForMarketplace<T>,
	) -> Result<(), Error<T>>;

	/// Locks aka reduces available storage capacity by `registration`s required amount.
	fn lock(source: &T::AccountId, registration: &JobRegistrationFor<T>) -> Result<(), Error<T>>;

	/// Unlocks aka increases available storage capacity by `registration`s required amount.
	fn unlock(source: &T::AccountId, registration: &JobRegistrationFor<T>) -> Result<(), Error<T>>;
}

/// Weight functions needed for pallet_acurast_marketplace.
pub trait WeightInfo {
	fn advertise() -> Weight;
	fn delete_advertisement() -> Weight;
	fn report() -> Weight;
	fn propose_matching(x: u32) -> Weight;
	fn propose_execution_matching(x: u32) -> Weight;
	fn acknowledge_match() -> Weight;
	fn acknowledge_execution_match() -> Weight;
	fn finalize_job() -> Weight;
	fn finalize_jobs(x: u32) -> Weight;
	fn cleanup_storage(x: u32) -> Weight;
	fn cleanup_assignments(x: u32) -> Weight;
}
