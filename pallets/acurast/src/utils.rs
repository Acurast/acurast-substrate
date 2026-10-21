use acurast_common::{
	error::ValidationError, validate_certificate_chain, AttestationSecurityLevel,
	BoundedAttestationContent, ECDSACurve, PublicKey, ValidatedChain,
};
use frame_support::{ensure, traits::UnixTime};
use parity_scale_codec::Encode;
use sp_std::prelude::*;

use crate::{
	Attestation, AttestationChain, AttestationValidity, CertId, Config, Error, IssuerName,
	KeyAttestationBarrier, ProcessorInfoProvider, ProcessorType, RkpCertificateProcessor,
	SerialNumber, StoredAttestation, StoredRevokedCertificate, ValidatingCertIds,
};

/// Validates and returns an [Attestation] from the provided chain.
pub fn validate_and_extract_attestation<T: Config>(
	source: &T::AccountId,
	attestation_chain: &AttestationChain,
) -> Result<Attestation, Error<T>> {
	validate_and_extract_attestation_with_rkp_cert::<T>(source, attestation_chain)
		.map(|(attestation, _)| attestation)
}

/// Validates and returns an [Attestation] from the provided chain, with its RKP device certificate if it has one.
pub(crate) fn validate_and_extract_attestation_with_rkp_cert<T: Config>(
	source: &T::AccountId,
	attestation_chain: &AttestationChain,
) -> Result<(Attestation, Option<CertId>), Error<T>> {
	let ValidatedChain { cert_ids, validity, public_key, is_rkp, attestation } =
		validate_certificate_chain(&attestation_chain.certificate_chain, now::<T>()?).map_err(
			|error| match error {
				ValidationError::CertificateNotValidAtTime => {
					Error::<T>::AttestationCertificateNotValid
				},
				ValidationError::BuildNotApproved => Error::<T>::AttestationBuildNotApproved,
				ValidationError::TooManyCertificatesIssued => {
					Error::<T>::AttestationTooManyCertificatesIssued
				},
				ValidationError::ExtractAttestation => Error::<T>::AttestationExtractionFailed,
				_ => Error::<T>::CertificateChainValidationFailed,
			},
		)?;

	ensure_valid_public_key_for_source(source, &public_key)?;

	let attestation_validity = AttestationValidity {
		not_before: validity
			.not_before
			.timestamp_millis()
			.map_err(|_| Error::<T>::FailedTimestampConversion)?,
		not_after: validity
			.not_after
			.timestamp_millis()
			.map_err(|_| Error::<T>::FailedTimestampConversion)?,
	};

	let cert_ids_bounded = cert_ids
		.into_iter()
		.map(|cert_id| {
			let (iss, sn) = cert_id;
			let iss_bounded = IssuerName::try_from(iss)
				.map_err(|_| Error::<T>::CannotGetAttestationIssuerName)?;
			let sn_bounded = SerialNumber::try_from(sn)
				.map_err(|_| Error::<T>::CannotGetAttestationSerialNumber)?;
			Ok((iss_bounded, sn_bounded))
		})
		.collect::<Result<Vec<CertId>, Error<T>>>()?;
	let cert_ids_bounded_vec = ValidatingCertIds::try_from(cert_ids_bounded)
		.map_err(|_| Error::<T>::CannotGetCertificateId)?;

	let attestation = Attestation {
		cert_ids: cert_ids_bounded_vec,
		content: attestation
			.try_into()
			.map_err(|_| Error::<T>::AttestationToBoundedTypeConversionFailed)?,
		validity: attestation_validity,
	};
	let rkp_cert = if is_rkp { rkp_cert_id(&attestation).cloned() } else { None };
	Ok((attestation, rkp_cert))
}

/// Ensures that the provided account id has a valid (not expired and not revoked) key attestation.
pub fn ensure_source_verified<T: Config>(source: &T::AccountId) -> Result<(), Error<T>> {
	_ = check_attestation(source)?;
	Ok(())
}

pub fn ensure_source_verified_and_of_type<T: Config>(
	source: &T::AccountId,
	processor_type: ProcessorType,
) -> Result<(), Error<T>> {
	let attestation = check_attestation(source)?;
	if !T::KeyAttestationBarrier::check_attestation_is_of_type(&attestation, processor_type) {
		return Err(Error::<T>::AttestationRejected);
	}
	Ok(())
}

pub fn ensure_source_verified_and_security_level<T: Config>(
	account: &T::AccountId,
	valid_security_levels: &[AttestationSecurityLevel],
) -> Result<(), Error<T>> {
	let attestation = check_attestation(account)?;
	if let BoundedAttestationContent::KeyDescription(key_description) = attestation.content {
		if !valid_security_levels.contains(&key_description.attestation_security_level) {
			return Err(Error::<T>::AttestationRejected);
		}
	}
	Ok(())
}

fn now<T: Config>() -> Result<u64, Error<T>> {
	T::UnixTime::now()
		.as_millis()
		.try_into()
		.map_err(|_| Error::<T>::FailedTimestampConversion)
}

/// Ensures the attestation is not expired.
pub(crate) fn ensure_not_expired<T: Config>(attestation: &Attestation) -> Result<(), Error<T>> {
	let now = now::<T>()?;

	if now >= attestation.validity.not_after || now < attestation.validity.not_before {
		return Err(Error::<T>::AttestationCertificateNotValid);
	}

	if let BoundedAttestationContent::KeyDescription(key_description) = &attestation.content {
		let expire_date_time = key_description
			.tee_enforced
			.usage_expire_date_time
			.or(key_description.software_enforced.usage_expire_date_time);
		if let Some(expire_date_time) = expire_date_time {
			if now >= expire_date_time {
				return Err(Error::<T>::AttestationUsageExpired);
			}
		}
	}
	Ok(())
}

/// Ensures the attestation is not signed by a revoked certificate.
pub(crate) fn ensure_not_revoked<T: Config>(attestation: &Attestation) -> Result<(), Error<T>> {
	let ids = &attestation.cert_ids;
	for id in ids {
		if <StoredRevokedCertificate<T>>::get(&id.1).is_some() {
			return Err(Error::<T>::RevokedCertificate);
		}
	}
	Ok(())
}

/// The leaf's issuer of an Android attestation, which is the RKP device certificate in an RKP chain.
pub(crate) fn rkp_cert_id(attestation: &Attestation) -> Option<&CertId> {
	match attestation.content {
		BoundedAttestationContent::KeyDescription(_) => {
			attestation.cert_ids.get(attestation.cert_ids.len().checked_sub(2)?)
		},
		_ => None,
	}
}

/// Whether the stored attestation of `processor` is under the RKP device certificate `cert_id`.
pub(crate) fn holds_rkp_certificate<T: Config>(processor: &T::AccountId, cert_id: &CertId) -> bool {
	<StoredAttestation<T>>::get(processor)
		.is_some_and(|attestation| rkp_cert_id(&attestation) == Some(cert_id))
}

/// Ensures the RKP device certificate `cert_id` is not bound to another attested processor
/// paired with a manager other than `manager`.
pub(crate) fn ensure_rkp_certificate_available<T: Config>(
	cert_id: Option<&CertId>,
	account: &T::AccountId,
	manager: Option<&T::AccountId>,
) -> Result<(), Error<T>> {
	let Some(cert_id) = cert_id else {
		return Ok(());
	};
	let Some(bound) = <RkpCertificateProcessor<T>>::get(cert_id) else {
		return Ok(());
	};
	if &bound == account || !holds_rkp_certificate::<T>(&bound, cert_id) {
		return Ok(());
	}
	match T::ProcessorInfoProvider::manager(&bound) {
		Some(bound_manager) if Some(&bound_manager) != manager => {
			Err(Error::<T>::RkpCertificateInUse)
		},
		_ => Ok(()),
	}
}

fn check_attestation<T: Config>(account: &T::AccountId) -> Result<Attestation, Error<T>> {
	let attestation =
		<StoredAttestation<T>>::get(account).ok_or(Error::<T>::FulfillSourceNotVerified)?;
	ensure_not_revoked(&attestation)?;
	if !T::KeyAttestationBarrier::accept_attestation_for_origin(account, &attestation) {
		return Err(Error::<T>::AttestationRejected);
	}
	Ok(attestation)
}

/// Ensures the provided public key correponds to the provided account id.
fn ensure_valid_public_key_for_source<T: Config>(
	source: &T::AccountId,
	public_key: &PublicKey,
) -> Result<(), Error<T>> {
	match public_key {
		PublicKey::RSA(_) => Err(Error::<T>::UnsupportedAttestationPublicKeyType),
		PublicKey::ECDSA(public_key) => match public_key {
			ECDSACurve::CurveP256(public_key) => {
				let encoded_source = source.encode();
				let encoded_public_key =
					sp_io::hashing::blake2_256(&public_key.to_bytes()).to_vec();

				ensure!(
					encoded_source == encoded_public_key,
					Error::<T>::AttestationPublicKeyDoesNotMatchSource
				);
				Ok(())
			},
			ECDSACurve::CurveP384(_) => Err(Error::<T>::UnsupportedAttestationPublicKeyType),
		},
	}
}
