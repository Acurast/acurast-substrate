use acurast_common::{Attestation, JobId, MultiOrigin};
use frame_support::{dispatch::DispatchResultWithPostInfo, weights::Weight};
use sp_std::prelude::*;

use crate::{AllowedSourcesUpdate, Config, Error, JobRegistrationFor};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ProcessorType {
	Core,
	Lite,
}

/// Allows to customize the kind of key attestations that are accepted.
pub trait KeyAttestationBarrier<T: Config> {
	fn accept_attestation_for_origin(origin: &T::AccountId, attestation: &Attestation) -> bool;
	fn check_attestation_is_of_type(
		attestation: &Attestation,
		processor_type: ProcessorType,
	) -> bool;
}

impl<T: Config> KeyAttestationBarrier<T> for () {
	fn accept_attestation_for_origin(_origin: &<T>::AccountId, _attestation: &Attestation) -> bool {
		true
	}

	fn check_attestation_is_of_type(
		_attestation: &Attestation,
		_processor_type: ProcessorType,
	) -> bool {
		true
	}
}

/// Weight functions needed for pallet_acurast.
pub trait WeightInfo {
	fn register() -> Weight;
	fn deregister() -> Weight;
	fn update_allowed_sources(x: u32) -> Weight;
	fn submit_attestation() -> Weight;
	fn update_certificate_revocation_list() -> Weight;
	fn set_environment(x: u32) -> Weight;
	fn set_environments(envs: u32, vars: u32) -> Weight;
}

/// Allows to hook additional logic for various job related extrinsics.
pub trait JobHooks<T: Config> {
	fn register_hook(
		who: &MultiOrigin<T::AccountId>,
		job_id: &JobId<<T as frame_system::Config>::AccountId>,
		registration: &JobRegistrationFor<T>,
	) -> DispatchResultWithPostInfo;
	fn deregister_hook(
		job_id: &JobId<<T as frame_system::Config>::AccountId>,
	) -> DispatchResultWithPostInfo;
	fn update_allowed_sources_hook(
		who: &<T as frame_system::Config>::AccountId,
		job_id: &JobId<<T as frame_system::Config>::AccountId>,
		updates: &Vec<AllowedSourcesUpdate<<T as frame_system::Config>::AccountId>>,
	) -> DispatchResultWithPostInfo;
}

impl<T: Config> JobHooks<T> for () {
	fn register_hook(
		_who: &MultiOrigin<T::AccountId>,
		_job_id: &JobId<<T as frame_system::Config>::AccountId>,
		_registration: &JobRegistrationFor<T>,
	) -> DispatchResultWithPostInfo {
		Ok(().into())
	}
	fn deregister_hook(
		_job_id: &JobId<<T as frame_system::Config>::AccountId>,
	) -> DispatchResultWithPostInfo {
		Ok(().into())
	}
	fn update_allowed_sources_hook(
		_who: &<T as frame_system::Config>::AccountId,
		_job_id: &JobId<<T as frame_system::Config>::AccountId>,
		_updates: &Vec<AllowedSourcesUpdate<<T as frame_system::Config>::AccountId>>,
	) -> DispatchResultWithPostInfo {
		Ok(().into())
	}
}

impl<T: Config> From<()> for Error<T> {
	fn from(_: ()) -> Self {
		Self::JobHookFailed
	}
}
