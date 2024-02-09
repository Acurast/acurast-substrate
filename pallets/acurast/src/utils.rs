use acurast_common::{extract_attestation, validate_certificate_chain, ECDSACurve, PublicKey};
use codec::Encode;
use frame_support::{ensure, traits::UnixTime};
use sp_std::prelude::*;

use crate::{
    Attestation, AttestationChain, AttestationValidity, CertId, Config, Error, IssuerName,
    SerialNumber, StoredAttestation, StoredRevokedCertificate, ValidatingCertIds,
};

/// Validates and returns an [Attestation] from the provided chain.
pub fn validate_and_extract_attestation<T: Config>(
    source: &T::AccountId,
    attestation_chain: &AttestationChain,
) -> Result<Attestation, Error<T>> {
    let (cert_ids, cert, public_key) =
        validate_certificate_chain(&attestation_chain.certificate_chain)
            .map_err(|_| Error::<T>::CertificateChainValidationFailed)?;

    ensure_valid_public_key_for_source(source, &public_key)?;

    let attestation_validity = AttestationValidity {
        not_before: cert.validity.not_before.timestamp_millis(),
        not_after: cert.validity.not_after.timestamp_millis(),
    };

    let key_description = extract_attestation(cert.extensions)
        .map_err(|_| Error::<T>::AttestationExtractionFailed)?;

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

    Ok(Attestation {
        cert_ids: cert_ids_bounded_vec,
        key_description: key_description
            .try_into()
            .map_err(|_| Error::<T>::AttestationToBoundedTypeConversionFailed)?,
        validity: attestation_validity,
    })
}

/// Ensures that the provided account id has a valid (not expired and not revoked) key attestation.
pub fn ensure_source_verified<T: Config>(source: &T::AccountId) -> Result<(), Error<T>> {
    let attestation =
        <StoredAttestation<T>>::get(source).ok_or(Error::<T>::FulfillSourceNotVerified)?;
    ensure_not_expired(&attestation)?;
    ensure_not_revoked(&attestation)?;
    Ok(())
}

/// Ensures the attestation is not expired.
pub(crate) fn ensure_not_expired<T: Config>(attestation: &Attestation) -> Result<(), Error<T>> {
    let now: u64 = T::UnixTime::now()
        .as_millis()
        .try_into()
        .map_err(|_| Error::<T>::FailedTimestampConversion)?;

    if now >= attestation.validity.not_after || now < attestation.validity.not_before {
        return Err(Error::<T>::AttestationCertificateNotValid);
    }
    let expire_date_time = attestation
        .key_description
        .tee_enforced
        .usage_expire_date_time
        .or({
            attestation
                .key_description
                .software_enforced
                .usage_expire_date_time
        });
    if let Some(expire_date_time) = expire_date_time {
        if now >= expire_date_time {
            return Err(Error::<T>::AttestationUsageExpired);
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
            }
            ECDSACurve::CurveP384(_) => Err(Error::<T>::UnsupportedAttestationPublicKeyType),
        },
    }
}
