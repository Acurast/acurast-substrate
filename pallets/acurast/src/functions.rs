use core::ops::AddAssign as _;
use frame_support::{
	dispatch::{DispatchResult, DispatchResultWithPostInfo},
	ensure,
	sp_runtime::{BoundedVec, DispatchError},
	traits::Get,
};
use sp_std::prelude::*;

use acurast_common::{
	is_valid_script, Attestation, AttestationChain, AttestationSecurityLevel, AttestationValidator,
	EnsureAttested, JobId, JobIdSequence, Metrics, MinMetric, MinMetrics,
};

use crate::{
	utils::{
		ensure_not_expired, ensure_not_revoked, ensure_source_verified_and_security_level,
		validate_and_extract_attestation,
	},
	Config, EnvironmentFor, Error, Event, ExecutionEnvironment, JobHooks, JobRegistrationFor,
	KeyAttestationBarrier, LocalJobIdSequence, Pallet, RequiredMinMetrics, StoredAttestation,
	StoredJobRegistration,
};

impl<T: Config> Pallet<T> {
	/// Get and update the next job identifier in the sequence.
	pub fn next_job_id() -> JobIdSequence {
		<LocalJobIdSequence<T>>::mutate(|job_id_seq| {
			job_id_seq.add_assign(1);
			*job_id_seq
		})
	}

	/// Registers a job for the given [`multi_origin`].
	///
	/// It assumes the caller was already authorized and is intended to be used from
	/// * The [`Self::register`] extrinsic of this pallet
	/// * An interchain communication protocol like Hyperdrive
	pub fn register_for(
		job_id: JobId<T::AccountId>,
		registration: JobRegistrationFor<T>,
		min_metrics: Option<Metrics>,
	) -> DispatchResultWithPostInfo {
		ensure!(is_valid_script(&registration.script), Error::<T>::InvalidScriptValue);
		if let Some(allowed_sources) = &registration.allowed_sources {
			let max_allowed_sources_len = T::MaxAllowedSources::get() as usize;
			ensure!(allowed_sources.len() > 0, Error::<T>::TooFewAllowedSources);
			ensure!(
				allowed_sources.len() <= max_allowed_sources_len,
				Error::<T>::TooManyAllowedSources
			);
		}

		<StoredJobRegistration<T>>::insert(&job_id.0, job_id.1, registration.clone());
		if let Some(metrics) = min_metrics {
			let metrics: MinMetrics = metrics
				.into_iter()
				.map(MinMetric::checked_from)
				.filter_map(|metric| metric)
				.collect::<Vec<MinMetric>>()
				.try_into()
				.map_err(|_| Error::<T>::TooManyMinMetrics)?;
			<RequiredMinMetrics<T>>::insert(&job_id, metrics);
		}

		<T as Config>::JobHooks::register_hook(&job_id, &registration)?;

		Self::deposit_event(Event::JobRegistrationStoredV2(job_id.clone()));
		Ok(().into())
	}

	pub fn deregister_for(job_id: JobId<T::AccountId>) -> DispatchResultWithPostInfo {
		<T as Config>::JobHooks::deregister_hook(&job_id)?;
		Self::clear_environment_for(&job_id);
		<StoredJobRegistration<T>>::remove(&job_id.0, job_id.1);
		<RequiredMinMetrics<T>>::remove(&job_id);
		Self::deposit_event(Event::JobRegistrationRemoved(job_id));
		Ok(().into())
	}

	pub fn set_environment_for(
		job_id: JobId<T::AccountId>,
		environments: BoundedVec<(T::AccountId, EnvironmentFor<T>), T::MaxSlots>,
	) -> Result<(), Error<T>> {
		for (source, env) in environments {
			let _registration = <StoredJobRegistration<T>>::get(&job_id.0, job_id.1)
				.ok_or(Error::<T>::JobRegistrationNotFound)?;
			<ExecutionEnvironment<T>>::insert(&job_id, &source, env);
		}

		Self::deposit_event(Event::ExecutionEnvironmentsUpdatedV2(job_id));

		Ok(())
	}

	pub fn clear_environment_for(job_id: &JobId<T::AccountId>) {
		let _ = <ExecutionEnvironment<T>>::clear_prefix(job_id, T::MaxSlots::get(), None);
	}
}

impl<T: Config> EnsureAttested<T::AccountId> for Pallet<T> {
	fn ensure_attested(account_id: &T::AccountId) -> DispatchResult {
		ensure_source_verified_and_security_level::<T>(
			account_id,
			&[AttestationSecurityLevel::StrongBox, AttestationSecurityLevel::TrustedEnvironemnt],
		)?;

		Ok(())
	}
}

impl<T: Config> AttestationValidator<T::AccountId> for Pallet<T> {
	fn validate(
		attestation_chain: &AttestationChain,
		account: &T::AccountId,
	) -> Result<Attestation, DispatchError> {
		ensure!(
			attestation_chain.certificate_chain.len() >= 2,
			Error::<T>::CertificateChainTooShort,
		);

		let attestation = validate_and_extract_attestation::<T>(account, &attestation_chain)?;

		if !T::KeyAttestationBarrier::accept_attestation_for_origin(account, &attestation) {
			#[cfg(not(feature = "runtime-benchmarks"))]
			return Err(Error::<T>::AttestationRejected.into());
		}

		ensure_not_expired::<T>(&attestation)?;
		ensure_not_revoked::<T>(&attestation)?;
		Ok(attestation)
	}

	fn validate_and_store(
		attestation_chain: AttestationChain,
		account: T::AccountId,
	) -> DispatchResult {
		let attestation = Self::validate(&attestation_chain, &account)?;
		<StoredAttestation<T>>::insert(&account, attestation);
		Self::deposit_event(Event::AttestationStoredV2(account));
		Ok(())
	}
}
