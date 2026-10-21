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
	CertId, EnsureAttested, JobId, JobIdSequence, Metrics, MinMetric, MinMetrics,
	OnProcessorUnpaired,
};

use crate::{
	utils::{
		ensure_not_expired, ensure_not_revoked, ensure_rkp_certificate_available,
		ensure_source_verified_and_security_level, holds_rkp_certificate, rkp_cert_id,
		validate_and_extract_attestation_with_rkp_cert,
	},
	Config, EnvironmentFor, Error, Event, ExecutionEnvironment, JobHooks, JobRegistrationFor,
	KeyAttestationBarrier, LocalJobIdSequence, Pallet, RequiredMinMetrics, RkpCertificateProcessor,
	StoredAttestation, StoredJobRegistration,
};

impl<T: Config> Pallet<T> {
	/// Removes the stored attestation of `processor` and the binding of its RKP device certificate.
	pub(crate) fn remove_attestation(processor: &T::AccountId) {
		let Some(attestation) = <StoredAttestation<T>>::take(processor) else {
			return;
		};
		if let Some(cert_id) = rkp_cert_id(&attestation) {
			if <RkpCertificateProcessor<T>>::get(cert_id).as_ref() == Some(processor) {
				<RkpCertificateProcessor<T>>::remove(cert_id);
			}
		}
	}

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
			ensure!(!allowed_sources.is_empty(), Error::<T>::TooFewAllowedSources);
			ensure!(
				allowed_sources.len() <= max_allowed_sources_len,
				Error::<T>::TooManyAllowedSources
			);
		}

		<StoredJobRegistration<T>>::insert(&job_id.0, job_id.1, registration.clone());
		if let Some(metrics) = min_metrics {
			let metrics: MinMetrics = metrics
				.into_iter()
				.filter_map(MinMetric::checked_from)
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

impl<T: Config> Pallet<T> {
	/// Validates the chain of `account`, to be paired with `manager`, and returns the attestation with its RKP
	/// device certificate if it has one.
	fn validate_with_rkp_cert(
		attestation_chain: &AttestationChain,
		account: &T::AccountId,
		manager: Option<&T::AccountId>,
	) -> Result<(Attestation, Option<CertId>), DispatchError> {
		ensure!(
			attestation_chain.certificate_chain.len() >= 2,
			Error::<T>::CertificateChainTooShort,
		);

		let (attestation, rkp_cert) =
			validate_and_extract_attestation_with_rkp_cert::<T>(account, attestation_chain)?;

		if !T::KeyAttestationBarrier::accept_attestation_for_origin(account, &attestation) {
			#[cfg(not(feature = "runtime-benchmarks"))]
			return Err(Error::<T>::AttestationRejected.into());
		}

		ensure_not_expired::<T>(&attestation)?;
		ensure_not_revoked::<T>(&attestation)?;
		ensure_rkp_certificate_available::<T>(rkp_cert.as_ref(), account, manager)?;
		Ok((attestation, rkp_cert))
	}
}

impl<T: Config> AttestationValidator<T::AccountId> for Pallet<T> {
	fn validate(
		attestation_chain: &AttestationChain,
		account: &T::AccountId,
		manager: Option<&T::AccountId>,
	) -> Result<Attestation, DispatchError> {
		Self::validate_with_rkp_cert(attestation_chain, account, manager)
			.map(|(attestation, _)| attestation)
	}

	/// Validates and stores the attestation. A processor still attested under the same RKP device certificate
	/// loses its attestation; [Self::validate] ensures it is unpaired or paired with `manager`.
	fn validate_and_store(
		attestation_chain: AttestationChain,
		account: T::AccountId,
		manager: Option<&T::AccountId>,
	) -> DispatchResult {
		let (attestation, rkp_cert) =
			Self::validate_with_rkp_cert(&attestation_chain, &account, manager)?;
		Self::remove_attestation(&account);
		if let Some(cert_id) = rkp_cert {
			if let Some(bound) = <RkpCertificateProcessor<T>>::get(&cert_id) {
				if bound != account && holds_rkp_certificate::<T>(&bound, &cert_id) {
					Self::remove_attestation(&bound);
					Self::deposit_event(Event::ProcessorAttestationReplaced(
						bound,
						account.clone(),
					));
				}
			}
			<RkpCertificateProcessor<T>>::insert(&cert_id, &account);
		}
		<StoredAttestation<T>>::insert(&account, attestation);
		Self::deposit_event(Event::AttestationStoredV2(account));
		Ok(())
	}
}

impl<T: Config> OnProcessorUnpaired<T::AccountId> for Pallet<T> {
	fn processor_unpaired(processor: &T::AccountId, _former_manager: &T::AccountId) {
		Self::remove_attestation(processor);
	}
}
