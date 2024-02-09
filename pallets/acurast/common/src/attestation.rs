#![cfg_attr(all(feature = "alloc", not(feature = "std"), not(test)), no_std)]

pub mod asn;
pub mod error;

use asn::*;
use asn1::{oid, BitString, ObjectIdentifier, ParseError, SequenceOf};
use core::cell::RefCell;
use ecdsa_vendored::hazmat::VerifyPrimitive;
use error::ValidationError;
use frame_support::{traits::ConstU32, BoundedVec};
use num_bigint::BigUint;
use p256::ecdsa::{signature::Verifier, VerifyingKey};

use sha2::Digest;
use sp_std::prelude::*;

pub const CHAIN_MAX_LENGTH: u32 = 5;
pub const CERT_MAX_LENGTH: u32 = 3000;
pub type CertificateInput = BoundedVec<u8, ConstU32<CERT_MAX_LENGTH>>;
pub type CertificateChainInput = BoundedVec<CertificateInput, ConstU32<CHAIN_MAX_LENGTH>>;

fn parse_cert(serialized: &[u8]) -> Result<Certificate, ParseError> {
    let data = asn1::parse_single::<Certificate>(serialized)?;
    Ok(data)
}

fn parse_cert_payload(serialized: &[u8]) -> Result<&[u8], ParseError> {
    let payload = asn1::parse_single::<CertificateRawPayload>(serialized)?;

    Ok(payload.tbs_certificate.full_data())
}

pub type CertificateId = (Vec<u8>, Vec<u8>);

/// Creates a unique id for a certificate.
pub fn unique_id(
    issuer: &Name,
    serial_number: &asn1::BigUint,
) -> Result<CertificateId, ValidationError> {
    let issuer_encoded = asn1::write_single(issuer).map_err(|_| ValidationError::InvalidIssuer)?;
    let serial_number_encoded = serial_number.as_bytes().to_vec();
    Ok((issuer_encoded, serial_number_encoded))
}

/// The OID of the Attestation Extension to a X.509 certificate.
/// [See docs](https://source.android.com/docs/security/keystore/attestation#tbscertificate-sequence)
pub const KEY_ATTESTATION_OID: ObjectIdentifier = oid!(1, 3, 6, 1, 4, 1, 11129, 2, 1, 17);

/// Extracts and parses the attestation from the extension field of a X.509 certificate.
pub fn extract_attestation<'a>(
    extensions: Option<SequenceOf<'a, Extension<'a>>>,
) -> Result<KeyDescription<'a>, ValidationError> {
    let extension = extensions
        .ok_or(ValidationError::ExtensionMissing)?
        .find(|e| e.extn_id == KEY_ATTESTATION_OID)
        .ok_or(ValidationError::ExtensionMissing)?;

    let version = peek_attestation_version(extension.extn_value)?;

    match version {
        1 => {
            let parsed = asn1::parse_single::<KeyDescriptionV1>(extension.extn_value)?;
            Ok(KeyDescription::V1(parsed))
        }
        2 => {
            let parsed = asn1::parse_single::<KeyDescriptionV2>(extension.extn_value)?;
            Ok(KeyDescription::V2(parsed))
        }
        3 => {
            let parsed = asn1::parse_single::<KeyDescriptionV3>(extension.extn_value)?;
            Ok(KeyDescription::V3(parsed))
        }
        4 => {
            let parsed = asn1::parse_single::<KeyDescriptionV4>(extension.extn_value)?;
            Ok(KeyDescription::V4(parsed))
        }
        100 => {
            let parsed = asn1::parse_single::<KeyDescriptionKeyMint>(extension.extn_value)?;
            Ok(KeyDescription::V100(parsed))
        }
        200 => {
            let parsed = asn1::parse_single::<KeyDescriptionKeyMint>(extension.extn_value)?;
            Ok(KeyDescription::V200(parsed))
        }
        300 => {
            let parsed = asn1::parse_single::<KeyDescriptionKeyMint>(extension.extn_value)?;
            Ok(KeyDescription::V300(parsed))
        }
        _ => Err(ValidationError::UnsupportedAttestationVersion(version)),
    }
}

const RSA_ALGORITHM: ObjectIdentifier = oid!(1, 2, 840, 113549, 1, 1, 11);
const ECDSA_WITH_SHA256_ALGORITHM: ObjectIdentifier = oid!(1, 2, 840, 10045, 4, 3, 2); // https://oidref.com/1.2.840.10045.4.3.2
const ECDSA_WITH_SHA384_ALGORITHM: ObjectIdentifier = oid!(1, 2, 840, 10045, 4, 3, 3); // https://oidref.com/1.2.840.10045.4.3.3

const RSA_PBK: ObjectIdentifier = oid!(1, 2, 840, 113549, 1, 1, 1);
const ECDSA_PBK: ObjectIdentifier = oid!(1, 2, 840, 10045, 2, 1);

#[derive(Clone, Eq, PartialEq)]
pub enum PublicKey {
    RSA(RSAPbk),
    ECDSA(ECDSACurve),
}

#[derive(Clone, Eq, PartialEq)]
pub struct RSAPbk {
    exponent: BigUint,
    modulus: BigUint,
}

#[derive(Clone, Eq, PartialEq)]
pub enum ECDSACurve {
    CurveP256(VerifyingKey),
    CurveP384(p384::AffinePoint),
}

impl PublicKey {
    fn parse(info: &SubjectPublicKeyInfo) -> Result<Self, ValidationError> {
        match &info.algorithm.algorithm {
            &RSA_PBK => {
                let pbk = parse_rsa_pbk(info.subject_public_key.as_bytes())?;
                Ok(PublicKey::RSA(pbk))
            }
            &ECDSA_PBK => {
                let pbk_param = info
                    .algorithm
                    .parameters
                    .ok_or(ValidationError::MissingECDSAAlgorithmTyp)?;
                let typ = asn1::parse_single::<ObjectIdentifier>(pbk_param.full_data())?;
                match typ {
                    CURVE_P256 => {
                        let verifying_key =
                            VerifyingKey::from_sec1_bytes(&info.subject_public_key.as_bytes())
                                .or(Err(ValidationError::ParseP256PublicKey))?;
                        Ok(PublicKey::ECDSA(ECDSACurve::CurveP256(verifying_key)))
                    }
                    CURVE_P384 => {
                        // the first byte tells us if compressed or not, we always assume uncompressed and ignore it.
                        let encoded = &info.subject_public_key.as_bytes()[1..];
                        let middle = encoded.len() / 2;
                        let point = p384::AffinePoint {
                            x: p384::FieldElement::from_be_slice(&encoded[..middle])?,
                            y: p384::FieldElement::from_be_slice(&encoded[middle..])?,
                            infinity: 0,
                        };
                        Ok(PublicKey::ECDSA(ECDSACurve::CurveP384(point)))
                    }
                    _ => Result::Err(ValidationError::UnsupportedSignatureAlgorithm)?,
                }
            }
            _ => Result::Err(ValidationError::UnsupportedPublicKeyAlgorithm),
        }
    }
}

const CURVE_P256: ObjectIdentifier = oid!(1, 2, 840, 10045, 3, 1, 7);
const CURVE_P384: ObjectIdentifier = oid!(1, 3, 132, 0, 34);

fn validate<'a>(
    cert: &Certificate<'a>,
    payload: &[u8],
    pbk: &PublicKey,
) -> Result<(), ValidationError> {
    if cert.signature_algorithm.algorithm != cert.tbs_certificate.signature.algorithm {
        return Err(ValidationError::SignatureMismatch);
    }
    match cert.signature_algorithm.algorithm {
        RSA_ALGORITHM => match pbk {
            PublicKey::RSA(pbk) => validate_rsa(&payload, &cert.signature_value, &pbk),
            _ => Err(ValidationError::UnsupportedPublicKeyAlgorithm),
        },
        ECDSA_WITH_SHA256_ALGORITHM => match pbk {
            PublicKey::ECDSA(pbk) => {
                validate_ecdsa::<sha2::Sha256>(&payload, &cert.signature_value, &pbk)
            }
            _ => Err(ValidationError::UnsupportedPublicKeyAlgorithm),
        },
        ECDSA_WITH_SHA384_ALGORITHM => match pbk {
            PublicKey::ECDSA(pbk) => {
                validate_ecdsa::<sha2::Sha384>(&payload, &cert.signature_value, &pbk)
            }
            _ => Err(ValidationError::UnsupportedPublicKeyAlgorithm),
        },
        _ => Err(ValidationError::UnsupportedSignatureAlgorithm)?,
    }
}

fn validate_rsa(
    payload: &[u8],
    signature: &BitString,
    pbk: &RSAPbk,
) -> Result<(), ValidationError> {
    let computed = {
        let signature_num = BigUint::from_bytes_be(signature.as_bytes());
        let computed = signature_num.modpow(&pbk.exponent, &pbk.modulus);
        computed.to_bytes_be()
    };

    // read hash digest and consume hasher
    let hashed = &sha2::Sha256::digest(payload)[..];

    let unpadded = &computed[computed.len() - hashed.len()..];

    if hashed != unpadded {
        return Err(ValidationError::InvalidSignature);
    }

    Ok(())
}

fn validate_ecdsa<D>(
    payload: &[u8],
    signature: &BitString,
    curve: &ECDSACurve,
) -> Result<(), ValidationError>
where
    D: Digest,
{
    match curve {
        ECDSACurve::CurveP256(verifying_key) => {
            let signature = p256::ecdsa::Signature::from_der(&signature.as_bytes())
                .or(Err(ValidationError::InvalidSignatureEncoding))?;
            verifying_key
                .verify(payload, &signature)
                .or(Err(ValidationError::InvalidSignature))?;
        }
        ECDSACurve::CurveP384(affine_point) => {
            let signature = ecdsa_vendored::Signature::from_der(signature.as_bytes())
                .or(Err(ValidationError::InvalidSignatureEncoding))?;

            let hashed = &D::digest(payload);
            let mut padded: [u8; 48] = [0; 48];
            if hashed.len() == 32 {
                padded[16..].copy_from_slice(hashed);
            } else {
                padded.copy_from_slice(hashed);
            }
            let payload = p384::FieldBytes::from_slice(&padded);

            affine_point
                .verify_prehashed(*payload, &signature)
                .or(Err(ValidationError::InvalidSignature))?;
        }
    };

    Ok(())
}

fn parse_rsa_pbk(data: &[u8]) -> Result<RSAPbk, ParseError> {
    let pbk = asn1::parse_single::<RSAPublicKey>(data)?;
    Ok(RSAPbk {
        exponent: BigUint::from_bytes_be(pbk.exponent.as_bytes()),
        modulus: BigUint::from_bytes_be(pbk.modulus.as_bytes()),
    })
}

pub fn peek_attestation_version(data: &[u8]) -> Result<i64, ParseError> {
    let result: asn1::ParseResult<_> = asn1::parse(data, |d| {
        // as we are not reading the sequence to the end, the parser always returns an error result
        // therefore setup a cell to store the result and ignore result
        let attestation_version: RefCell<i64> = RefCell::from(0);
        let _: Result<_, ParseError> = d.read_element::<asn1::Sequence>()?.parse(|d| {
            *attestation_version.borrow_mut() = d.read_element::<i64>()?;
            // this gets always covered by parse error
            Ok(())
        });

        Ok(attestation_version.into_inner())
    });
    result
}

/// Validates the chain by ensuring that
///
/// - the chain starts with a self-signed certificate at index 0 that matches one of the known [TRUSTED_ROOT_CERTS]
/// - that the root's contained public key signs the next certificate in the chain
/// - the next certificate's public key signs the next one and so on...
pub fn validate_certificate_chain<'a>(
    chain: &'a CertificateChainInput,
) -> Result<(Vec<CertificateId>, TBSCertificate<'a>, PublicKey), ValidationError> {
    let root_pub_key = PublicKey::parse(&asn1::parse_single::<SubjectPublicKeyInfo>(
        TRUSTED_ROOT_PUB_KEY,
    )?)?;
    let mut cert_ids = Vec::<CertificateId>::new();
    let fold_result = chain.iter().try_fold::<_, _, Result<_, ValidationError>>(
        (Option::<PublicKey>::None, Option::<Certificate>::None),
        |(prev_pbk, _), cert_data| {
            let cert = parse_cert(&cert_data)?;
            let payload = parse_cert_payload(&cert_data)?;
            let current_pbk = PublicKey::parse(&cert.tbs_certificate.subject_public_key_info)?;
            if prev_pbk.is_none() && current_pbk != root_pub_key {
                return Err(ValidationError::UntrustedRoot);
            }

            validate(&cert, payload, prev_pbk.as_ref().unwrap_or(&current_pbk))?;

            let unique_id = unique_id(
                &cert.tbs_certificate.issuer,
                &cert.tbs_certificate.serial_number,
            )?;
            cert_ids.push(unique_id);

            // it's crucial for security to pass on a non-null public key here,
            // otherwise self-signed certificates would get accepted later down the chain
            Ok((Some(current_pbk), Some(cert)))
        },
    )?;

    let last_cert = fold_result.1.ok_or(ValidationError::ChainTooShort)?;
    let last_cert_pbk = fold_result.0.ok_or(ValidationError::MissingPublicKey)?;

    // if the chain is non-empty as ensured above, we know that we always have Some certificate in option
    Ok((cert_ids, last_cert.tbs_certificate, last_cert_pbk))
}

const TRUSTED_ROOT_PUB_KEY: &'static [u8] = include_bytes!("./__root_key__/public.key");

#[cfg(test)]
mod tests {
    use core::convert::TryInto;

    use crate::{
        attestation::{error::ValidationError, extract_attestation},
        BoundedKeyDescription,
    };

    use super::{
        asn::KeyDescription, validate_certificate_chain, CertificateChainInput, CertificateInput,
    };

    pub fn decode_certificate_chain(chain: &Vec<&str>) -> CertificateChainInput {
        let decoded = chain
            .iter()
            .map(|cert_data| {
                CertificateInput::truncate_from(
                    base64::decode(&cert_data).expect("error decoding test input"),
                )
            })
            .collect::<Vec<CertificateInput>>();
        CertificateChainInput::truncate_from(decoded)
    }

    const SAMSUNG_ROOT_CERT: &str = r"MIIFHDCCAwSgAwIBAgIJANUP8luj8tazMA0GCSqGSIb3DQEBCwUAMBsxGTAXBgNVBAUTEGY5MjAwOWU4NTNiNmIwNDUwHhcNMTkxMTIyMjAzNzU4WhcNMzQxMTE4MjAzNzU4WjAbMRkwFwYDVQQFExBmOTIwMDllODUzYjZiMDQ1MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAr7bHgiuxpwHsK7Qui8xUFmOr75gvMsd/dTEDDJdSSxtf6An7xyqpRR90PL2abxM1dEqlXnf2tqw1Ne4Xwl5jlRfdnJLmN0pTy/4lj4/7tv0Sk3iiKkypnEUtR6WfMgH0QZfKHM1+di+y9TFRtv6y//0rb+T+W8a9nsNL/ggjnar86461qO0rOs2cXjp3kOG1FEJ5MVmFmBGtnrKpa73XpXyTqRxB/M0n1n/W9nGqC4FSYa04T6N5RIZGBN2z2MT5IKGbFlbC8UrW0DxW7AYImQQcHtGl/m00QLVWutHQoVJYnFPlXTcHYvASLu+RhhsbDmxMgJJ0mcDpvsC4PjvB+TxywElgS70vE0XmLD+OJtvsBslHZvPBKCOdT0MS+tgSOIfga+z1Z1g7+DVagf7quvmag8jfPioyKvxnK/EgsTUVi2ghzq8wm27ud/mIM7AY2qEORR8Go3TVB4HzWQgpZrt3i5MIlCaY504LzSRiigHCzAPlHws+W0rB5N+er5/2pJKnfBSDiCiFAVtCLOZ7gLiMm0jhO2B6tUXHI/+MRPjy02i59lINMRRev56GKtcd9qO/0kUJWdZTdA2XoS82ixPvZtXQpUpuL12ab+9EaDK8Z4RHJYYfCT3Q5vNAXaiWQ+8PTWm2QgBR/bkwSWc+NpUFgNPN9PvQi8WEg5UmAGMCAwEAAaNjMGEwHQYDVR0OBBYEFDZh4QB8iAUJUYtEbEf/GkzJ6k8SMB8GA1UdIwQYMBaAFDZh4QB8iAUJUYtEbEf/GkzJ6k8SMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgIEMA0GCSqGSIb3DQEBCwUAA4ICAQBOMaBc8oumXb2voc7XCWnuXKhBBK3e2KMGz39t7lA3XXRe2ZLLAkLM5y3J7tURkf5a1SutfdOyXAmeE6SRo83Uh6WszodmMkxK5GM4JGrnt4pBisu5igXEydaW7qq2CdC6DOGjG+mEkN8/TA6p3cnoL/sPyz6evdjLlSeJ8rFBH6xWyIZCbrcpYEJzXaUOEaxxXxgYz5/cTiVKN2M1G2okQBUIYSY6bjEL4aUN5cfo7ogP3UvliEo3Eo0YgwuzR2v0KR6C1cZqZJSTnghIC/vAD32KdNQ+c3N+vl2OTsUVMC1GiWkngNx1OO1+kXW+YTnnTUOtOIswUP/Vqd5SYgAImMAfY8U9/iIgkQj6T2W6FsScy94IN9fFhE1UtzmLoBIuUFsVXJMTz+Jucth+IqoWFua9v1R93/k98p41pjtFX+H8DslVgfP097vju4KDlqN64xV1grw3ZLl4CiOe/A91oeLm2UHOq6wn3esB4r2EIQKb6jTVGu5sYCcdWpXr0AUVqcABPdgL+H7qJguBw09ojm6xNIrw2OocrDKsudk/okr/AwqEyPKw9WnMlQgLIKw1rODG2NvU9oR3GVGdMkUBZutL8VuFkERQGt6vQ2OCw0sV47VMkuYbacK/xyZFiRcrPJPb41zgbQj9XAEyLKCHex0SdDrx+tWUDqG8At2JHA==";
    const SAMSUNG_KEY_CERT: &str = r"MIIClzCCAj2gAwIBAgIBATAKBggqhkjOPQQDAjA5MQwwCgYDVQQMDANURUUxKTAnBgNVBAUTIGIyYzM3ZTM4MzI4ZDZhY2RmM2I2MDA2ZThhNzdmMDY0MB4XDTIxMTExNzIyNDcxMloXDTMxMTExNTIyNDcxMlowHzEdMBsGA1UEAxMUQW5kcm9pZCBLZXlzdG9yZSBLZXkwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAASDWA5xIavYEzjbcZneQy8gxkAo7nzJrSIqHbmPDy1kOFNWidIZLaKf86qLp73/n2VzK8qo5XsHexoC8wPaIcj8o4IBTjCCAUowggE2BgorBgEEAdZ5AgERBIIBJjCCASICAWQKAQECAWQKAQEEAAQAMGy/hT0IAgYBgddgKwm/hUVcBFowWDEyMDAEK2NvbS51YmluZXRpYy5hdHRlc3RlZC5leGVjdXRvci50ZXN0LnRlc3RuZXQCAQ4xIgQgvctFYPazxB2tkgZoFpwovh756knyPZjNjrLzeuRIj/kwgaGhBTEDAgECogMCAQOjBAICAQClBTEDAgEAqgMCAQG/g3cCBQC/hT4DAgEAv4VATDBKBCDnyVk+0qoHM1jC6eS+ScTwsvI1J6mtlFgzf0F3HTIMawEB/woBAAQgowcEEJQaU4V58HU/EPyCMBydcLlh8pR+qgnfWnuur+W/hUEFAgMB1MC/hUIFAgMDFdy/hU4GAgQBNInxv4VPBgIEATSJ8TAOBgNVHQ8BAf8EBAMCB4AwCgYIKoZIzj0EAwIDSAAwRQIgOQNrjHRHg9gcN6gFJFZHSjpIG1Gx1061FAEq3E9yUsgCIQD1FvhmjYsTWeQMQsj22ms/8dw9O3WsvE0y2AtrN0KWuw==";
    const SAMSUNG_INTERMEDIATE_1_CERT: &str = r"MIIB8zCCAXmgAwIBAgIQcH2ewbAt6vTdz/WwWLWu6zAKBggqhkjOPQQDAjA5MQwwCgYDVQQMDANURUUxKTAnBgNVBAUTIDgxYjU3ZmZmYjM3OTUxMjljZjNmYzUwZWNhMGNkMzljMB4XDTIxMTExNzIyNDcxMloXDTMxMTExNTIyNDcxMlowOTEMMAoGA1UEDAwDVEVFMSkwJwYDVQQFEyBiMmMzN2UzODMyOGQ2YWNkZjNiNjAwNmU4YTc3ZjA2NDBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABE3rCk6dqUilYhf1gsiVMFkOrEze/Ar318VMXFXDlOXDajQORIGWYVVtbcHYPNrews45k2CgHZg6ofN4lpONImyjYzBhMB0GA1UdDgQWBBRt1zXt/O233wIFRiNawaRD3KQPpTAfBgNVHSMEGDAWgBQNE845gvrI02p2mda2mk3SWwhGYjAPBgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwICBDAKBggqhkjOPQQDAgNoADBlAjEA0dNMiUn0+ftvhsFJP1byGMZkaWWOQbIOTItcQTrw29YV5FSjwZW7Ofrj8kR8WC4nAjB0yDVyt86uFrvWWzaa1EJmqR4L7PMUWf8yVey6KLrhQYMSGGhgief4pj3Hx6Eck6o=";
    const SAMSUNG_INTERMEDIATE_2_CERT: &str = r"MIIDlDCCAXygAwIBAgIRAJ3uw09QZQdXUqFIiXyf5uUwDQYJKoZIhvcNAQELBQAwGzEZMBcGA1UEBRMQZjkyMDA5ZTg1M2I2YjA0NTAeFw0yMTExMTcyMjQ1MTBaFw0zMTExMTUyMjQ1MTBaMDkxDDAKBgNVBAwMA1RFRTEpMCcGA1UEBRMgODFiNTdmZmZiMzc5NTEyOWNmM2ZjNTBlY2EwY2QzOWMwdjAQBgcqhkjOPQIBBgUrgQQAIgNiAARSfOriwm02QddIzGI1JpbUWTw93rtxu/BBMGpQopLCEsI1IMcO+YO75XEx5PJb0qpN0qZy4ZyohEOkXyqdD/KNkNCKWnhVk7wyyJCdnw35L8+adMpuHkp7Wc8nK14aXKKjYzBhMB0GA1UdDgQWBBQNE845gvrI02p2mda2mk3SWwhGYjAfBgNVHSMEGDAWgBQ2YeEAfIgFCVGLRGxH/xpMyepPEjAPBgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwICBDANBgkqhkiG9w0BAQsFAAOCAgEAVRzcron3lJ+sG5Jaqd9L2G33Dm/0/u0Ed+1jNJ7LrCLMKSHmEmoEiuNRKue2Tyv8UVb/Z9dENmC+gBqWkgOB6hxJ6lVcvIa38/CKNHBHr/Ras55+zZ68tQlpO6tdOVKUlfvlvI1BdpCv4qSEMpR9Zz4f4dzjEAbb24isT0PLcYvN0IrDELdCK+R+b+HaM5GrcFj1STv3uju/xHJnU6GeMdMPFf/rbMLNi1P6xVqdNUBGbKFx8J+px78z/Bcjq8Swt+uEoINvk/whROT8TQuzdccofx0hRFaoC1lgjRo8xgLlqFIyj0ICETuyYfEXbJwGgJczdS7ndte2SES4Rl3+NlYA2/mXjBUPnmGvJraOUZaw7ahIay7L7uUpvdJCHrlCDpRSLLCjuNss/sGn6bb3EDVGBaqzNRUBLNbsqrwKf8MbaJMhxOzHFlVXO1heFvmVdB+69Gkf0Kt2fK8N6VJIDGI9YoluItIbgJ/IqCicwLduxqMSXpPHEXf+f0lQH/AAP6Gz0aD4on3qTjPSl8p4LOqZSQoDqJKUukaXhMvgr/4u4E3ZX3EbxrF77hrML4NK4DfOj3LjLklPZZ3cLlMXzcSnMYvXkVU96qHqppyqjfioOZU2oSFQwPbXmKIYHVYJ2xIFBVy9ESQcqX04mevxMh1YHp+pTdMLXYE0EU+lB5Q=";

    const PIXEL_ROOT_CERT: &str = r"MIIFYDCCA0igAwIBAgIJAOj6GWMU0voYMA0GCSqGSIb3DQEBCwUAMBsxGTAXBgNVBAUTEGY5MjAwOWU4NTNiNmIwNDUwHhcNMTYwNTI2MTYyODUyWhcNMjYwNTI0MTYyODUyWjAbMRkwFwYDVQQFExBmOTIwMDllODUzYjZiMDQ1MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAr7bHgiuxpwHsK7Qui8xUFmOr75gvMsd/dTEDDJdSSxtf6An7xyqpRR90PL2abxM1dEqlXnf2tqw1Ne4Xwl5jlRfdnJLmN0pTy/4lj4/7tv0Sk3iiKkypnEUtR6WfMgH0QZfKHM1+di+y9TFRtv6y//0rb+T+W8a9nsNL/ggjnar86461qO0rOs2cXjp3kOG1FEJ5MVmFmBGtnrKpa73XpXyTqRxB/M0n1n/W9nGqC4FSYa04T6N5RIZGBN2z2MT5IKGbFlbC8UrW0DxW7AYImQQcHtGl/m00QLVWutHQoVJYnFPlXTcHYvASLu+RhhsbDmxMgJJ0mcDpvsC4PjvB+TxywElgS70vE0XmLD+OJtvsBslHZvPBKCOdT0MS+tgSOIfga+z1Z1g7+DVagf7quvmag8jfPioyKvxnK/EgsTUVi2ghzq8wm27ud/mIM7AY2qEORR8Go3TVB4HzWQgpZrt3i5MIlCaY504LzSRiigHCzAPlHws+W0rB5N+er5/2pJKnfBSDiCiFAVtCLOZ7gLiMm0jhO2B6tUXHI/+MRPjy02i59lINMRRev56GKtcd9qO/0kUJWdZTdA2XoS82ixPvZtXQpUpuL12ab+9EaDK8Z4RHJYYfCT3Q5vNAXaiWQ+8PTWm2QgBR/bkwSWc+NpUFgNPN9PvQi8WEg5UmAGMCAwEAAaOBpjCBozAdBgNVHQ4EFgQUNmHhAHyIBQlRi0RsR/8aTMnqTxIwHwYDVR0jBBgwFoAUNmHhAHyIBQlRi0RsR/8aTMnqTxIwDwYDVR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMCAYYwQAYDVR0fBDkwNzA1oDOgMYYvaHR0cHM6Ly9hbmRyb2lkLmdvb2dsZWFwaXMuY29tL2F0dGVzdGF0aW9uL2NybC8wDQYJKoZIhvcNAQELBQADggIBACDIw41L3KlXG0aMiS//cqrG+EShHUGo8HNsw30W1kJtjn6UBwRM6jnmiwfBPb8VA91chb2vssAtX2zbTvqBJ9+LBPGCdw/E53Rbf86qhxKaiAHOjpvAy5Y3m00mqC0w/Zwvju1twb4vhLaJ5NkUJYsUS7rmJKHHBnETLi8GFqiEsqTWpG/6ibYCv7rYDBJDcR9W62BW9jfIoBQcxUCUJouMPH25lLNcDc1ssqvC2v7iUgI9LeoM1sNovqPmQUiG9rHli1vXxzCyaMTjwftkJLkf6724DFhuKug2jITV0QkXvaJWF4nUaHOTNA4uJU9WDvZLI1j83A+/xnAJUucIv/zGJ1AMH2boHqF8CY16LpsYgBt6tKxxWH00XcyDCdW2KlBCeqbQPcsFmWyWugxdcekhYsAWyoSf818NUsZdBWBaR/OukXrNLfkQ79IyZohZbvabO/X+MVT3rriAoKc8oE2Uws6DF+60PV7/WIPjNvXySdqspImSN78mflxDqwLqRBYkA3I75qppLGG9rp7UCdRjxMl8ZDBld+7yvHVgt1cVzJx9xnyGCC23UaicMDSXYrB4I4WHXPGjxhZuCuPBLTdOLU8YRvMYdEvYebWHMpvwGCF6bAx3JBpIeOQ1wDB5y0USicV3YgYGmi+NZfhA4URSh77Yd6uuJOJENRaNVTzk";
    const PIXEL_INTERMEDIATE_2_CERT: &str = r"MIID1zCCAb+gAwIBAgIKA4gmZ2BliZaF9TANBgkqhkiG9w0BAQsFADAbMRkwFwYDVQQFExBmOTIwMDllODUzYjZiMDQ1MB4XDTE5MDgwOTIzMDMyM1oXDTI5MDgwNjIzMDMyM1owLzEZMBcGA1UEBRMQNTRmNTkzNzA1NDJmNWE5NTESMBAGA1UEDAwJU3Ryb25nQm94MHYwEAYHKoZIzj0CAQYFK4EEACIDYgAE41Inb5v86kMBpfBCf6ZHjlcyCa5E/XYs+8V8u9RxNjFQnoAuoOlAU25U+iVwyihGFUaYB1UJKTsxALOVW0MXdosoa/b+JlHFmvbGsNszYAkKRkfHhg527MO4p9tc5XrMo4G2MIGzMB0GA1UdDgQWBBRpkLEMOwiK7ir4jDOHtCwS2t/DpjAfBgNVHSMEGDAWgBQ2YeEAfIgFCVGLRGxH/xpMyepPEjAPBgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwICBDBQBgNVHR8ESTBHMEWgQ6BBhj9odHRwczovL2FuZHJvaWQuZ29vZ2xlYXBpcy5jb20vYXR0ZXN0YXRpb24vY3JsLzhGNjczNEM5RkE1MDQ3ODkwDQYJKoZIhvcNAQELBQADggIBAFxZEyegsCSeytyUkYTJZR7R8qYXoXUWQ5h1Qp6b0h+H/SNl0NzedHAiwZQQ8jqzgP4c7w9HrrxEPCpFMd8+ykEBv5bWvDDf2HjtZzRlMRG154KgM1DMJgXhKLSKV+f/H+S/QQTeP3yprOavsBvdkgX6ELkYN6M3JXr7gpCvpFb6Ypz65Ud7FysAm/KNQ9zU0x7cvz3Btvz8ylw4p5dz04tanTzNgVLVHyX5kAcB2ftPvxMH4X/PXdx1lAmGPS8PsubCRGjJxdhRVOEEMYyxCuYLonuyUggOByZFaBw55WDoWGpkVQhnFi9L3p23VkWILLnq/07+GwoxL1vUAiQpjJHxNQYbjgTo+kxhjDP3uULAKPANGBE7+25VqVLMtdce4Eb5v9yFqgg+JtlL41RUWVS3DIEqxOMm/fB3A7t55TbUKf8dCZyBci2BcUWTx8K7VnQMy8gBMyu1SGleKPLIrBRSomDP5X8xGtwTLo3aAdY4+aSjEoimI6kX9bbIfhyDFpJxKaDRHzhCUdLfJrlCp2hEq5GWj0lT50hPLs0tbhh/l3LTtFhKyYbiB5vHXyB3P4gUui0WxyZnYdajUF+Tn8MW79qHhwhaXU9HnflE+dBh0smazOc+0xdwZZKXET+UFAUAMGiHvhuICCuWsY4SPKv8/715toeCoECHSMv08C9C";
    const PIXEL_INTERMEDIATE_1_CERT: &str = r"MIICMDCCAbegAwIBAgIKFZBYV0ZxdmNYNDAKBggqhkjOPQQDAjAvMRkwFwYDVQQFExA1NGY1OTM3MDU0MmY1YTk1MRIwEAYDVQQMDAlTdHJvbmdCb3gwHhcNMTkwNzI3MDE1MjE5WhcNMjkwNzI0MDE1MjE5WjAvMRkwFwYDVQQFExA5NzM1Mzc3OTM2ZDBkZDc0MRIwEAYDVQQMDAlTdHJvbmdCb3gwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAAR2OZY6u30za18jjYs1Xv2zlaIrLM3me9okMo5Lv4Av76l/IE3YvbRQMyy15Wb3Wb3G/6+587x443R9/Ognjl8Co4G6MIG3MB0GA1UdDgQWBBRBPjyps0vHpRy7ASXAQhvmUa162DAfBgNVHSMEGDAWgBRpkLEMOwiK7ir4jDOHtCwS2t/DpjAPBgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwICBDBUBgNVHR8ETTBLMEmgR6BFhkNodHRwczovL2FuZHJvaWQuZ29vZ2xlYXBpcy5jb20vYXR0ZXN0YXRpb24vY3JsLzE1OTA1ODU3NDY3MTc2NjM1ODM0MAoGCCqGSM49BAMCA2cAMGQCMBeg3ziAoi6h1LPfvbbASk5WVdC6cL3IpaxIOycMHm1SDNqYALOtd1uujfzMeobs+AIwKJj5XySGe7MRL0QNtdrSd2nkK+fbjcUc8LKvVapDwRAC40CiTzllAy+aOnyDxrvb";
    const PIXEL_KEY_CERT: &str = r"MIICnDCCAkGgAwIBAgIBATAMBggqhkjOPQQDAgUAMC8xGTAXBgNVBAUTEDk3MzUzNzc5MzZkMGRkNzQxEjAQBgNVBAwMCVN0cm9uZ0JveDAiGA8yMDIyMDcwOTEwNTE1NVoYDzIwMjgwNTIzMjM1OTU5WjAfMR0wGwYDVQQDDBRBbmRyb2lkIEtleXN0b3JlIEtleTBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABLIMHRVHdmJiPs9DAQSJgAbg+BwNsbrofLlqh8d3dARlnlhdPZBXuKL/iuYfQBoHj8dc9SyMQmjoEPk3mMcp6GKjggFWMIIBUjAOBgNVHQ8BAf8EBAMCB4AwggE+BgorBgEEAdZ5AgERBIIBLjCCASoCAQQKAQICASkKAQIECHRlc3Rhc2RmBAAwbL+FPQgCBgGB4pZhH7+FRVwEWjBYMTIwMAQrY29tLnViaW5ldGljLmF0dGVzdGVkLmV4ZWN1dG9yLnRlc3QudGVzdG5ldAIBDjEiBCC9y0Vg9rPEHa2SBmgWnCi+HvnqSfI9mM2OsvN65EiP+TCBoaEFMQMCAQKiAwIBA6MEAgIBAKUFMQMCAQCqAwIBAb+DdwIFAL+FPgMCAQC/hUBMMEoEIIec0/GOp24kTU1Kw7y5wzfBO0ZnGQsZA1r+JTZVAFDxAQH/CgEABCA/QTbuNYHmq6jqM3prQ9cD3h7KJB+bfyd+zfr/96jc8b+FQQUCAwHUwL+FQgUCAwMV3r+FTgYCBAE0ir2/hU8GAgQBNIq9MAwGCCqGSM49BAMCBQADRwAwRAIgM6YTzOmm7SUCakkrZR8Kxnw8AonU5HQxaMaQPi+qC9oCIDJM01xL8mldca0Sooho5pIyESki6vDjaZ9q3YEz1SjZ";

    const PIXEL_KEY_CERT_INVALID: &str = r"MIICnDCCAkGgAwIBAgIBATAMBggqhkjOPQQDAgUAMC8xGTAXBgNVBAUTEDk3MzUzNzc5MzZkMGRkNzQxEjAQBgNVBAwMCVN0cm9uZ0JveDAiGA8yMDIyMDcwOTEwNTE1NVoYDzIwMjgwNTIzMjM1OTU5WjAfMR0wGwYDVQQDDBRBbmRyb2lkIEtleXN0b3JlIEtleTBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABLIMHRVHdmJiPs9DAQSJgAbg+BwNsbrofLlqh8d3dARlnlhdPZBXuKL/iuYfQBoHj8dc9SyMQmjoEPk3mMcp6GKjggFWMIIBUjAOBgNVHQ8BAf8EBAMCB4AwggE+BgorBgEEAdZ5AgERBIIBLjCCASoCAQQKAQICASkKAQIECHRlc3Rhc2RmBAAwbL+FPQgCBgGB4pZhH7+FRVwEWjBYMTIwMAQrY29tLnViaW5ldGljLmF0dGVzdGVkLmV4ZWN1dG9yLnRlc3QudGVzdG5ldAIBDjEiBCC9y0Vg9rPEHa2SBmgWnCi+HvnqSfI9mM2OsvN65EiP+TCBoaEFMQMCAQKiAwIBA6MEAgIBAKUFMQMCAQCqAwIBAb+DdwIFAL+FPgMCAQC/hUBMMEoEIIec0/GOp24kTU1Kw7y5wzfBO0ZnGQsZA1r+JTZVAFDxAQH/CgEABCA/QTbuNYHmq6jqM3prQ9cD3h7KJB+bfyd+zfr/96jc8b+FQQUCAwHUwL+FQgUCAwMV3r+FTgYCBAE0ir2/hU8GAgQBNIq9MAwGCCqGSM49BAMCBQADRwAwRAIgM6YTzOmm7SUCakkrZR8Kxnw8AonU5HQxaMaQPi+qC9oCIDJM01xL8mldca0Sooho5pIyESki6vDjaZ9q3YAz1SjZ";
    const PIXEL_ROOT_CERT_UNTRUSTED: &str = r"MIIFYDCCA0igAwIBAgIJAOj6GWMU0voYMA0GCSqGSIb3DQEBCwUAMBsxGTAXBgNVBAUTEGY5MjAwOWU4NTNiNmIwNDUwHhcNMTYwNTI2MTYyODUyWhcNMjYwNTI0MTYyODUyWjAbMRkwFwYDVQQFExBmOTIwMDllODUzYjZiMDQ1MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAr7bHgiuxpwHsK7Qui8xUFmOr75gvMsd/dTEDDJdSSxtf6An7xyqpRR90PL2abxM1dEqlXnf2tqw1Ne4Xwl5jlRfdnJLmN0pTy/4lj4/7tv0Sk3iiKkypnEUtR6WfMgH0QZfKHM1+di+y9TFRtv6y//0rb+T+W8a9nsNL/ggjnar86461qO0rOs2cXjp3kOG1FEJ5MVmFmBGtnrKpa73XpXyTqRxB/M0n1n/W9nGqC4FSYa04T6N5RIZGBN2z2MT5IKGbFlbC8UrW0DxW7AYImQQcHtGl/m00QLVWutHQoVJYnFPlXTcHYvASLu+RhhsbDmxMgJJ0mcDpvsC4PjvB+TxywElgS70vE0XmLD+OJtvsBslHZvPBKCOdT0MS+tgSOIfga+z1Z1g7+DVagf7quvmag8jfPioyKvxnK/EgsTUVi2ghzq8wm27ud/mIM7AY2qEORR8Go3TVB4HzWQgpZrt3i5MIlCaY504LzSRiigHCzAPlHws+W0rB5N+er5/2pJKnfBSDiCiFAVtCLOZ7gLiMm0jhO2B6tUXHI/+MRPjy02i59lINMRRev56GKtcd9qO/0kUJWdZTdA2XoS82ixPvZtXQpUpuL12ab+9EaDK8Z4RHJYYfCT3Q5vNAXaiWQ+8PTWm2QgBR/bkwSWc+NpUFgNPN9PvQi8WEg5UmAGMCAwEAAaOBpjCBozAdBgNVHQ4EFgQUNmHhAHyIBQlRi0RsR/8aTMnqTxIwHwYDVR0jBBgwFoAUNmHhAHyIBQlRi0RsR/8aTMnqTxIwDwYDVR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMCAYYwQAYDVR0fBDkwNzA1oDOgMYYvaHR0cHM6Ly9hbmRyb2lkLmdvb2dsZWFwaXMuY29tL2F0dGVzdGF0aW9uL2NybC8wDQYJKoZIhvcNAQELBQADggIBACDIw41L3KlXG0aMiS//cqrG+EShHUGo8HNsw30W1kJtjn6UBwRM6jnmiwfBPb8VA91chb2vssAtX2zbTvqBJ9+LBPGCdw/E53Rbf86qhxKaiAHOjpvAy5Y3m00mqC0w/Zwvju1twb4vhLaJ5NkUJYsUS7rmJKHHBnETLi8GFqiEsqTWpG/6ibYCv7rYDBJDcR9W62BW9jfIoBQcxUCUJouMPH25lLNcDc1ssqvC2v7iUgI9LeoM1sNovqPmQUiG9rHli1vXxzCyaMTjwftkJLkf6724DFhuKug2jITV0QkXvaJWF4nUaHOTNA4uJU9WDvZLI1j83A+/xnAJUucIv/zGJ1AMH2boHqF8CY16LpsYgBt6tKxxWH00XcyDCdW2KlBCeqbQPcsFmWyWugxdcekhYsAWyoSf818NUsZdBWBaR/OukXrNLfkQ79IyZohZbvabO/X+MVT3rriAoKc8oE2Uws6DF+60PV7/WIPjNvXySdqspImSN78mflxDqwLqRBYkA3I75qppLGG9rp7UCdRjxMl8ZDBld+7yvHVgt1cVzJx9xnyGCC23UaicMDSXYrB4I4WHXPGjxhZuCuPBLTdOLU8YRvMYdEvYebWHMpvwGCF6bAx3JBpIeOQ1wDB5y0USicV3YgYGmi+NZfhA4URSh77Yd6uuJOJENRaNVTzl";

    type Error = ();

    impl From<ValidationError> for Error {
        fn from(_: ValidationError) -> Self {
            ()
        }
    }

    #[test]
    fn test_validate_samsung_chain() -> Result<(), Error> {
        let chain = vec![
            SAMSUNG_ROOT_CERT,
            SAMSUNG_INTERMEDIATE_2_CERT,
            SAMSUNG_INTERMEDIATE_1_CERT,
            SAMSUNG_KEY_CERT,
        ];
        let decoded_chain = decode_certificate_chain(&chain);
        let (_, cert, _) = validate_certificate_chain(&decoded_chain)?;
        let key_description = extract_attestation(cert.extensions)?;
        match &key_description {
            KeyDescription::V100(key_description) => {
                assert_eq!(key_description.attestation_version, 100)
            }
            _ => return Err(()),
        }
        let _: BoundedKeyDescription = key_description.try_into()?;
        Ok(())
    }

    #[test]
    fn test_validate_samsung_chain_2() -> Result<(), Error> {
        let decoded_chain = [
            hex_literal::hex!("3082051c30820304a003020102020900d50ff25ba3f2d6b3300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3139313132323230333735385a170d3334313131383230333735385a301b31193017060355040513106639323030396538353362366230343530820222300d06092a864886f70d01010105000382020f003082020a0282020100afb6c7822bb1a701ec2bb42e8bcc541663abef982f32c77f7531030c97524b1b5fe809fbc72aa9451f743cbd9a6f1335744aa55e77f6b6ac3535ee17c25e639517dd9c92e6374a53cbfe258f8ffbb6fd129378a22a4ca99c452d47a59f3201f44197ca1ccd7e762fb2f53151b6feb2fffd2b6fe4fe5bc6bd9ec34bfe08239daafceb8eb5a8ed2b3acd9c5e3a7790e1b51442793159859811ad9eb2a96bbdd7a57c93a91c41fccd27d67fd6f671aa0b815261ad384fa37944864604ddb3d8c4f920a19b1656c2f14ad6d03c56ec060899041c1ed1a5fe6d3440b556bad1d0a152589c53e55d370762f0122eef91861b1b0e6c4c80927499c0e9bec0b83e3bc1f93c72c049604bbd2f1345e62c3f8e26dbec06c94766f3c128239d4f4312fad8123887e06becf567583bf8355a81feeabaf99a83c8df3e2a322afc672bf120b135158b6821ceaf309b6eee77f98833b018daa10e451f06a374d50781f359082966bb778b9308942698e74e0bcd24628a01c2cc03e51f0b3e5b4ac1e4df9eaf9ff6a492a77c1483882885015b422ce67b80b88c9b48e13b607ab545c723ff8c44f8f2d368b9f6520d31145ebf9e862ad71df6a3bfd2450959d653740d97a12f368b13ef66d5d0a54a6e2f5d9a6fef446832bc67844725861f093dd0e6f3405da89643ef0f4d69b6420051fdb93049673e36950580d3cdf4fbd08bc58483952600630203010001a3633061301d0603551d0e041604143661e1007c880509518b446c47ff1a4cc9ea4f12301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff040403020204300d06092a864886f70d01010b050003820201004e31a05cf28ba65dbdafa1ced70969ee5ca84104added8a306cf7f6dee50375d745ed992cb0242cce72dc9eed51191fe5ad52bad7dd3b25c099e13a491a3cdd487a5acce8766324c4ae46338246ae7b78a418acbb98a05c4c9d696eeaab609d0ba0ce1a31be98490df3f4c0ea9ddc9e82ffb0fcb3e9ebdd8cb952789f2b1411fac56c886426eb7296042735da50e11ac715f1818cf9fdc4e254a3763351b6a2440150861263a6e310be1a50de5c7e8ee880fdd4be5884a37128d18830bb3476bf4291e82d5c66a6494939e08480bfbc00f7d8a74d43e73737ebe5d8e4ec515302d4689692780dc7538ed7e9175be6139e74d43ad388b3050ffd5a9de5262000898c01f63c53dfe22209108fa4f65ba16c49ccbde0837d7c5844d54b7398ba0122e505b155c9313cfe26e72d87e22aa1616e6bdbf547ddff93df29e35a63b455fe1fc0ec95581f3f4f7bbe3bb828396a37ae3157582bc3764b9780a239efc0f75a1e2e6d941ceabac27ddeb01e2bd8421029bea34d51aee6c60271d5a95ebd00515a9c0013dd80bf87eea260b81c34f688e6eb1348af0d8ea1cac32acb9d93fa24aff030a84c8f2b0f569cc95080b20ac35ace0c6d8dbd4f6847719519d32450166eb4bf15b859044501adeaf436382c34b15e3b54c92e61b69c2bfc7264589172b3c93dbe35ce06d08fd5c01322ca0877b1d12743af1fad5940ea1bc02dd891c").to_vec().try_into().unwrap(),
            hex_literal::hex!("3082039930820181a00302010202105f641897b6861d5994a20e80ba2a7b08300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3231303931353232353733305a170d3331303931333232353733305a303f31123010060355040c0c095374726f6e67426f78312930270603550405132031383464643461376438613731333165393861616534656137616635313064613076301006072a8648ce3d020106052b8104002203620004e242634c3b59ae131662e099fec272ec720d7642cdcc042f5f605cffddbe73b36822d2181fd27684afc25ba59983efa91a60f901a0097545163a7654c662f62e9059d97cc14e592d5efc69ec25cac3ccbda903fd511b1c4b13825d2f19e92a59a3633061301d0603551d0e0416041448e9cf69260a89c4c584499c6dbad846e5a266b3301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff040403020204300d06092a864886f70d01010b050003820201004931dd12fbf627e89b0cce272e83b0071ca0538b42d8001e34f8ddc710fba237c7040361520769fda10a17501db710dc1894ccbf1fff6f4bcedff0a7708fe6b40fa456b9f528d46dcc04a2167b7c130f649ac1f1756794438b315d15877fe790253f13e4023979bb0688bc44af9f09eeee1a4d9f0339f3508f689ba24026c7e59e5b527740762fa87e76d70b6ab24ad4fb2f2d7dd4ac4d0b69226a9a8a124a3bfe5319f16c530aec2508537dec72ea7bc9d0b75eb4cccc70844912d8089a56b21d6d64a1ae55c3ae6c5f9a3a8bbed5d0c64501071082f1df33997bbc5bdb1461fcf5eeb3ffc39f47d15b24d3bdd3904963b37197b6773cfe314b94a653f691ba5294cf6df110b627100d1579f7b6ef8b3b643d0606e072869de7ce6d5d6fde1ff7ee60c251b0f633cb20203fe8243857b0fbcecfcd92f8dd6db31aa5022c695fa912f4de1782e2c42c32aa8e7bf4f8d2626cc151336871fde59f85ce66a193a7a2d8e3529ef03a21ee4bdbc2880772831cf0fa53a301f11efa1951eaaca5c5a7740d1ab5e206b411b27f23cd93fab10e0ee40c08ef61fb8d18e521eb867fc169e64af8be6e79fb5923c706541bfe44cf76bf7a6774f3b4b9e8b9876eca4a1bd896490856f61f1678d06c176dafc228eeb797052b4b24d320879e1ee5181f763a4061cf77f9b5034a8ce317ea7ce1b8d10061d14d39acf8fdb463dbcd44ac5524").to_vec().try_into().unwrap(),
            hex_literal::hex!("308201fe30820185a00302010202106fd912a55cb7563380ddfa2ac2d1eb8c300a06082a8648ce3d040302303f31123010060355040c0c095374726f6e67426f7831293027060355040513203138346464346137643861373133316539386161653465613761663531306461301e170d3231303931353232353831375a170d3331303931333232353831375a303f31123010060355040c0c095374726f6e67426f78312930270603550405132037386235656561343461313465353437373261363862653665623738313462343059301306072a8648ce3d020106082a8648ce3d03010703420004df1afbe9c0d1fb4d291a6573de2cdf95ca9f43e4bd2f33ded358ca8118dd42ea619a9cdbccd4cc32335f5db51774bb5e9ccf08a2d6d3259572a27fe064fd451da3633061301d0603551d0e04160414f40c71002bcb6425158db7b4b552d09820d9e4c9301f0603551d2304183016801448e9cf69260a89c4c584499c6dbad846e5a266b3300f0603551d130101ff040530030101ff300e0603551d0f0101ff040403020204300a06082a8648ce3d0403020367003064023044f91fe977e5ddba9c69a4f0b0d9b7c550e8e117216d6768b3477e4277fb88e9ed1dcae08c538b5452f30f4aabd473c10230312efaac05f998263283e0b718025eaa224ee8e386b6f143b9781c789f3a38abc8480731325893d6efc25b67c473ecf8").to_vec().try_into().unwrap(),
            hex_literal::hex!("308202ba30820260a003020102020101300a06082a8648ce3d040302303f31123010060355040c0c095374726f6e67426f7831293027060355040513203738623565656134346131346535343737326136386265366562373831346234301e170d3730303130313030303030305a170d3439313233313233353935395a301f311d301b06035504031314416e64726f6964204b657973746f7265204b65793059301306072a8648ce3d020106082a8648ce3d03010703420004e203b4ed148733aca6322978ffb9d72dc940919d489d87ae5242cf6eb39b6ae8ab9edf67310b5b88e8ab5e82f7ec6cb7778ad00c480e4846aa6dbbde55303b43a382016b30820167300c0603551d0f0405030307880030820155060a2b06010401d67902011104820145308201410201640a01020201640a0102042052331675bfbad8106ff561287fcb0397307dc4ed8cccc3f7e2ace4a7fe82f19c04003065bf853d080206018c1a85f13ebf85455504533051312b30290424636f6d2e616375726173742e61747465737465642e6578656375746f722e63616e61727902011031220420ec70c2a4e072a0f586552a68357b23697c9d45f1e1257a8c4d29a25ac49824333081a7a10b3109020106020103020102a203020103a30402020100a5053103020100aa03020101bf8377020500bf853e03020100bf85404c304a0420c276f9fcf895a8838c8d6e6ec441494822e69acfca3bb27715790b2951da33980101ff0a010004209b51df228d989cd600b59e307b0bbc1d92013f969d4cebcc0ca3e1556bff6e7bbf854105020301fbd0bf854205020303163fbf854e0602040134b09dbf854f0602040134b09d300a06082a8648ce3d040302034800304502200229006e3a528c45224739b7773731c99ca811fa3ee57121626bbc9279fad3af022100bf47d355feae07bb6a4528e8872a8775248d4960477ce807cf4ee7ce902d42b9").to_vec().try_into().unwrap(),
        ].to_vec().try_into().unwrap();
        let (_, cert, _) = validate_certificate_chain(&decoded_chain)?;
        let key_description = extract_attestation(cert.extensions)?;
        match &key_description {
            KeyDescription::V100(key_description) => {
                assert_eq!(key_description.attestation_version, 100)
            }
            _ => return Err(()),
        }
        let _: BoundedKeyDescription = key_description.try_into()?;
        Ok(())
    }

    #[test]
    fn test_validate_pixel_devices() -> Result<(), Error> {
        let chains = vec![
            vec![
                r"MIIFHDCCAwSgAwIBAgIJANUP8luj8tazMA0GCSqGSIb3DQEBCwUAMBsxGTAXBgNVBAUTEGY5MjAwOWU4NTNiNmIwNDUwHhcNMTkxMTIyMjAzNzU4WhcNMzQxMTE4MjAzNzU4WjAbMRkwFwYDVQQFExBmOTIwMDllODUzYjZiMDQ1MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAr7bHgiuxpwHsK7Qui8xUFmOr75gvMsd/dTEDDJdSSxtf6An7xyqpRR90PL2abxM1dEqlXnf2tqw1Ne4Xwl5jlRfdnJLmN0pTy/4lj4/7tv0Sk3iiKkypnEUtR6WfMgH0QZfKHM1+di+y9TFRtv6y//0rb+T+W8a9nsNL/ggjnar86461qO0rOs2cXjp3kOG1FEJ5MVmFmBGtnrKpa73XpXyTqRxB/M0n1n/W9nGqC4FSYa04T6N5RIZGBN2z2MT5IKGbFlbC8UrW0DxW7AYImQQcHtGl/m00QLVWutHQoVJYnFPlXTcHYvASLu+RhhsbDmxMgJJ0mcDpvsC4PjvB+TxywElgS70vE0XmLD+OJtvsBslHZvPBKCOdT0MS+tgSOIfga+z1Z1g7+DVagf7quvmag8jfPioyKvxnK/EgsTUVi2ghzq8wm27ud/mIM7AY2qEORR8Go3TVB4HzWQgpZrt3i5MIlCaY504LzSRiigHCzAPlHws+W0rB5N+er5/2pJKnfBSDiCiFAVtCLOZ7gLiMm0jhO2B6tUXHI/+MRPjy02i59lINMRRev56GKtcd9qO/0kUJWdZTdA2XoS82ixPvZtXQpUpuL12ab+9EaDK8Z4RHJYYfCT3Q5vNAXaiWQ+8PTWm2QgBR/bkwSWc+NpUFgNPN9PvQi8WEg5UmAGMCAwEAAaNjMGEwHQYDVR0OBBYEFDZh4QB8iAUJUYtEbEf/GkzJ6k8SMB8GA1UdIwQYMBaAFDZh4QB8iAUJUYtEbEf/GkzJ6k8SMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgIEMA0GCSqGSIb3DQEBCwUAA4ICAQBOMaBc8oumXb2voc7XCWnuXKhBBK3e2KMGz39t7lA3XXRe2ZLLAkLM5y3J7tURkf5a1SutfdOyXAmeE6SRo83Uh6WszodmMkxK5GM4JGrnt4pBisu5igXEydaW7qq2CdC6DOGjG+mEkN8/TA6p3cnoL/sPyz6evdjLlSeJ8rFBH6xWyIZCbrcpYEJzXaUOEaxxXxgYz5/cTiVKN2M1G2okQBUIYSY6bjEL4aUN5cfo7ogP3UvliEo3Eo0YgwuzR2v0KR6C1cZqZJSTnghIC/vAD32KdNQ+c3N+vl2OTsUVMC1GiWkngNx1OO1+kXW+YTnnTUOtOIswUP/Vqd5SYgAImMAfY8U9/iIgkQj6T2W6FsScy94IN9fFhE1UtzmLoBIuUFsVXJMTz+Jucth+IqoWFua9v1R93/k98p41pjtFX+H8DslVgfP097vju4KDlqN64xV1grw3ZLl4CiOe/A91oeLm2UHOq6wn3esB4r2EIQKb6jTVGu5sYCcdWpXr0AUVqcABPdgL+H7qJguBw09ojm6xNIrw2OocrDKsudk/okr/AwqEyPKw9WnMlQgLIKw1rODG2NvU9oR3GVGdMkUBZutL8VuFkERQGt6vQ2OCw0sV47VMkuYbacK/xyZFiRcrPJPb41zgbQj9XAEyLKCHex0SdDrx+tWUDqG8At2JHA==",
                r"MIIDgDCCAWigAwIBAgIKA4gmZ2BliZaGDzANBgkqhkiG9w0BAQsFADAbMRkwFwYDVQQFExBmOTIwMDllODUzYjZiMDQ1MB4XDTIyMDEyNjIyNTAyMFoXDTM3MDEyMjIyNTAyMFowKTETMBEGA1UEChMKR29vZ2xlIExMQzESMBAGA1UEAxMJRHJvaWQgQ0EyMHYwEAYHKoZIzj0CAQYFK4EEACIDYgAE/t+4AI454D8pM32ZUEpuaS0ewLjFP9EBOnCF4Kkz2jqcDECp0fjy34AaTCgJnpGdCLIU3u/WXBs3pEECgMuS9RVSKqj584wdbpcxiJahZWSzHqPK1Nn5LZYdQIpLJ9cUo2YwZDAdBgNVHQ4EFgQUOZgHBjozEp71FAY6gEEMcYDOGq0wHwYDVR0jBBgwFoAUNmHhAHyIBQlRi0RsR/8aTMnqTxIwEgYDVR0TAQH/BAgwBgEB/wIBAjAOBgNVHQ8BAf8EBAMCAQYwDQYJKoZIhvcNAQELBQADggIBAD0FO58gwWQb6ROp4c7hkOwQiWiCTG2Ud9Nww5cKlsMU8YlZOk8nXn5OwAfuFT01Kgcbau1CNDECX7qA1vJyQ9HBsoqa7fmi0cf1j/RRBvvAuGvg3zRy0+OckwI2832399l/81FMShS+GczTWfhLJY/ObkVBFkanRCpDhE/SxNHL/5nJzYaH8OdjAKufnD9mcFyYvzjixbcPEO5melGwk7KfCx9miSpVuB6mN1NdoCsSi96ZYQGBlZsE8oLdazckCygTvp2s77GtIswywOHf3HEa39OQm8B8g2cHcy4u5kKoFeSPI9zo6jx+WDb1Er8gKZT1u7lrwCW+JUQquYbGHLzSDIsRfGh0sTjoRH/s4pD371OYAkkPMHVguBZE8iv5uv0j4IBwN/eLyoQb1jmBv/dEUU9ceXd/s8b5+8k7PYhYcDMA0oyFQcvrhLoWbqy7BrY25iWEY5xH6EsHFre5vp1su17Rdmxby3nt7mXz1NxBQdA3rM+kcZlfcK9sHTNVTI290Wy9IS+8/xalrtalo4PA6EwofyXy18XI9AddNs754KPf8/yAMbVc/2aClm1RF7/7vB0fx3eQmLE4WS01SsqsWnCsHCSbyjdIaIyKBFQhABtIIxLNYLFw+0nnA7DBU/M1e9gWBLh8dz1xHFo+Tn5edYaY1bYyhlGBKUKG4M8l",
                r"MIIB1jCCAVygAwIBAgITRmAX2O2EI8cs9AdsTEcYGXXWezAKBggqhkjOPQQDAzApMRMwEQYDVQQKEwpHb29nbGUgTExDMRIwEAYDVQQDEwlEcm9pZCBDQTIwHhcNMjMwMjIyMjEzMTU2WhcNMjMwMzI5MjEzMTU1WjApMRMwEQYDVQQKEwpHb29nbGUgTExDMRIwEAYDVQQDEwlEcm9pZCBDQTMwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAAQ0Zy87Qs6x388WTt5iKme8eIvZM22FlR9uW6U2A72JvBnVxqjy392XT0vLZ30mTeupy4MgHptYKG+Y480I4N0to2MwYTAOBgNVHQ8BAf8EBAMCAgQwDwYDVR0TAQH/BAUwAwEB/zAdBgNVHQ4EFgQUE27q90lSZLPa9fVy9tP75oNXXEMwHwYDVR0jBBgwFoAUOZgHBjozEp71FAY6gEEMcYDOGq0wCgYIKoZIzj0EAwMDaAAwZQIxAPDurBthkgVHiqSi0dT9I22gc3zJ0xqxy7FJzJzqaxn0sq2LbZdAGUBsUk59o4+0mQIwMP29yuexQCN8H4IAax4uMWVNQd2mfcptnH/PXg/7fg9ybGvJsqsk0hBCJHFHwTNi",
                r"MIIBwjCCAWmgAwIBAgIQN4fGiG3mLLKUUvIfLxgsajAKBggqhkjOPQQDAjApMRMwEQYDVQQKEwpHb29nbGUgTExDMRIwEAYDVQQDEwlEcm9pZCBDQTMwHhcNMjMwMjIxMDY1MTI2WhcNMjMwMzIzMDY1MTI2WjA5MQwwCgYDVQQKEwNURUUxKTAnBgNVBAMTIDM3ODdjNjg4NmRlNjJjYjI5NDUyZjIxZjJmMTgyYzZhMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAER8CSdyRFENwyJtIGF5rEhwlBaFBlmZgQ26Q2b/QJHTLFxCr1L1nhmO/wBGPPbH/8+rhKLgHEE+eJceOCT0YEjaNjMGEwHQYDVR0OBBYEFGYhzEPnZYjOCt6e18NI0qjt6+KwMB8GA1UdIwQYMBaAFBNu6vdJUmSz2vX1cvbT++aDV1xDMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgIEMAoGCCqGSM49BAMCA0cAMEQCIBLK+BdcEPgmFmHOMoysjfAshXISVllUw0uF+Qooz4E9AiAhnuNqKa+o3/zGQgxnbl+RtKWEqr5xOyou8XWzqf/FBA==",
                r"MIICtzCCAlygAwIBAgIBATAKBggqhkjOPQQDAjA5MQwwCgYDVQQKEwNURUUxKTAnBgNVBAMTIDM3ODdjNjg4NmRlNjJjYjI5NDUyZjIxZjJmMTgyYzZhMB4XDTcwMDEwMTAwMDAwMFoXDTQ4MDEwMTAwMDAwMFowHzEdMBsGA1UEAxMUQW5kcm9pZCBLZXlzdG9yZSBLZXkwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAAR1lprp4G3BY25RRGq2cfQEqzjNPKiEFJLZHzw+D3t06AcuiaJkTjmZX4WAaLO2KUh5SJTQ1jbboULujR+KwFCto4IBbTCCAWkwDgYDVR0PAQH/BAQDAgOIMIIBVQYKKwYBBAHWeQIBEQSCAUUwggFBAgIAyAoBAQICAMgKAQEEIO+euJ70aeZv9AxD3pZjmkhQYPYBi5ZmBnMZj3eiKpp+BAAwZr+FPQgCBgGGfQ1MP7+FRVYEVDBSMSwwKgQlY29tLmFjdXJhc3QuYXR0ZXN0ZWQuZXhlY3V0b3IudGVzdG5ldAIBDzEiBCCh29x/3ozMl7SQbY/OmbTZUOz2I4m75LOTmIzpBhtOkzCBpKEIMQYCAQICAQaiAwIBA6MEAgIBAKUFMQMCAQCqAwIBAb+DdwIFAL+FPgMCAQC/hUBMMEoEIJrEF0FT1F5FRbD0niL+Yyc5mbasHLaUnDqfA+yIB+7pAQH/CgEABCDaMHTYyuXAel8xl4iBT5opWtz9WzQzG4gudge6Q95Ku7+FQQUCAwH70L+FQgUCAwMV5L+FTgYCBAE0jRW/hU8GAgQBNI0VMAoGCCqGSM49BAMCA0kAMEYCIQCjcjDsMpH0ajNEl1siV3ia1BO4iIAxprX8sPFL8hQl3QIhAKrBWhDSmo/HgT9JSS+KagGBkiNrpa2VbrDQqYWQwS0d",
            ],
            vec![
                r"MIIFHDCCAwSgAwIBAgIJAPHBcqaZ6vUdMA0GCSqGSIb3DQEBCwUAMBsxGTAXBgNVBAUTEGY5MjAwOWU4NTNiNmIwNDUwHhcNMjIwMzIwMTgwNzQ4WhcNNDIwMzE1MTgwNzQ4WjAbMRkwFwYDVQQFExBmOTIwMDllODUzYjZiMDQ1MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAr7bHgiuxpwHsK7Qui8xUFmOr75gvMsd/dTEDDJdSSxtf6An7xyqpRR90PL2abxM1dEqlXnf2tqw1Ne4Xwl5jlRfdnJLmN0pTy/4lj4/7tv0Sk3iiKkypnEUtR6WfMgH0QZfKHM1+di+y9TFRtv6y//0rb+T+W8a9nsNL/ggjnar86461qO0rOs2cXjp3kOG1FEJ5MVmFmBGtnrKpa73XpXyTqRxB/M0n1n/W9nGqC4FSYa04T6N5RIZGBN2z2MT5IKGbFlbC8UrW0DxW7AYImQQcHtGl/m00QLVWutHQoVJYnFPlXTcHYvASLu+RhhsbDmxMgJJ0mcDpvsC4PjvB+TxywElgS70vE0XmLD+OJtvsBslHZvPBKCOdT0MS+tgSOIfga+z1Z1g7+DVagf7quvmag8jfPioyKvxnK/EgsTUVi2ghzq8wm27ud/mIM7AY2qEORR8Go3TVB4HzWQgpZrt3i5MIlCaY504LzSRiigHCzAPlHws+W0rB5N+er5/2pJKnfBSDiCiFAVtCLOZ7gLiMm0jhO2B6tUXHI/+MRPjy02i59lINMRRev56GKtcd9qO/0kUJWdZTdA2XoS82ixPvZtXQpUpuL12ab+9EaDK8Z4RHJYYfCT3Q5vNAXaiWQ+8PTWm2QgBR/bkwSWc+NpUFgNPN9PvQi8WEg5UmAGMCAwEAAaNjMGEwHQYDVR0OBBYEFDZh4QB8iAUJUYtEbEf/GkzJ6k8SMB8GA1UdIwQYMBaAFDZh4QB8iAUJUYtEbEf/GkzJ6k8SMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgIEMA0GCSqGSIb3DQEBCwUAA4ICAQB8cMqTllHc8U+qCrOlg3H7174lmaCsbo/bJ0C17JEgMLb4kvrqsXZs01U3mB/qABg/1t5Pd5AORHARs1hhqGICW/nKMav574f9rZN4PC2ZlufGXb7sIdJpGiO9ctRhiLuYuly10JccUZGEHpHSYM2GtkgYbZba6lsCPYAAP83cyDV+1aOkTf1RCp/lM0PKvmxYN10RYsK631jrleGdcdkxoSK//mSQbgcWnmAEZrzHoF1/0gso1HZgIn0YLzVhLSA/iXCX4QT2h3J5z3znluKG1nv8NQdxei2DIIhASWfu804CA96cQKTTlaae2fweqXjdN1/v2nqOhngNyz1361mFmr4XmaKH/ItTwOe72NI9ZcwS1lVaCvsIkTDCEXdm9rCNPAY10iTunIHFXRh+7KPzlHGewCq/8TOohBRn0/NNfh7uRslOSZ/xKbN9tMBtw37Z8d2vvnXq/YWdsm1+JLVwn6yYD/yacNJBlwpddla8eaVMjsF6nBnIgQOf9zKSe06nSTqvgwUHosgOECZJZ1EuzbH4yswbt02tKtKEFhx+v+OTge/06V+jGsqTWLsfrOCNLuA8H++z+pUENmpqnnHovaI47gC+TNpkgYGkkBT6B/m/U01BuOBBTzhIlMEZq9qkDWuM2cA5kW5V3FJUcfHnw1IdYIg2Wxg7yHcQZemFQg==",
                r"MIIDkzCCAXugAwIBAgIQD7Wk8tBKrvLKNmUKhwUEkzANBgkqhkiG9w0BAQsFADAbMRkwFwYDVQQFExBmOTIwMDllODUzYjZiMDQ1MB4XDTIyMDMyMDE5MDQ1MloXDTMyMDMxNzE5MDQ1MlowOTEMMAoGA1UEDAwDVEVFMSkwJwYDVQQFEyAzZGE5MzQ4MzVlZTQ3ZjdkZGQ4ZDg2NTA1MzliMGI1ZTB2MBAGByqGSM49AgEGBSuBBAAiA2IABHPhrAW8ysfh/31nOXLuPTkWs5u2gfxjvfPDxe5A/2y1sLLgQdJHRpdqMlJ3SamT67BfVQJI5DGasH4mfL8XJABXZ0eeBWVNAWimzOdm/ninHNikH969KqnDaoVulkUtYaNjMGEwHQYDVR0OBBYEFBCF11mfZEcWjpvLDRf6sP0q2UN2MB8GA1UdIwQYMBaAFDZh4QB8iAUJUYtEbEf/GkzJ6k8SMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgIEMA0GCSqGSIb3DQEBCwUAA4ICAQBPyQroXXPq+0t5UkwEcgiE+lJLyLBcIBU3f4+ri9KGLte/fpHten/o5HYvsMk4/hSpjnXdrafwMaGuLbYrcNXUgfMXsu4iCaF1nva7EwAinz69u91lvCTWtSKppOZ2Sio7Rg7tb+zhLXI9yT4RmDxhmhBfptqVoNVUMNJMAxMQ4NtkaqWiXjzb8rZybkMpd7g0Mtbr2sIBV3O5sFFTaMTC3b7bxh1pV42612pia2ooML3hU9ZpDIYhukrDs2fpc4Ke/oYHguvkiZCGhnYuReYCoz5Qcd9u8Z3bq5C54UiokMDQch8Vam2YPfk92Z+X0rsCvMM/ARFT0qWyfe9+JNd6VAmmtA9azBCJAKs1XyW/2kYdFq+WB7ApysqTI5UxNyRzAejNA2lBi/zEbTXx9dpt+Y/nkzTpYQi7hHJNNWiHVrIPcnyEamUmCvohLqS2aUPQ5MrMr9HhmafBZfPmgkRaKbwPHMQHyDBAkjMuSZevUUc1IFdmFPVkOFIFAx5/KB85ccc4uJ1YRHmqQ95pYmK6sXXOaqEljLQNUAWEa5iIR+mDCNoeUzmHcH41cC90sWOvkn6Y9WGjxKIfPpDRfML8SsW8DJ8Hn6hAlXBe2CG4JfcHaTADqI79JVcGi6GyWLqRrxOwbIGTS8UXvFX7q4dMpUwqLgO8psIkoQXqbg1vNA==",
                r"MIIB9DCCAXqgAwIBAgIRAPSwjDH2Lt8okFOXPtJH2iwwCgYIKoZIzj0EAwIwOTEMMAoGA1UEDAwDVEVFMSkwJwYDVQQFEyAzZGE5MzQ4MzVlZTQ3ZjdkZGQ4ZDg2NTA1MzliMGI1ZTAeFw0yMjAzMjAxOTA3NDFaFw0zMjAzMTcxOTA3NDFaMDkxDDAKBgNVBAwMA1RFRTEpMCcGA1UEBRMgMTZjYTIxNTQ3YTk4MGQ1ZDRiZmEyNmUyYmU5YjViZDIwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAAT0cXeZSQrTKndp7OCCZTlc1gEGNErjF408K9N92jqarW8Fxd/GO+IyiuN/HEWZ9pO8NCMca2yiQurzfgufnqLRo2MwYTAdBgNVHQ4EFgQU6HMm4aSkZ06ei9QqguVMzF3cUuYwHwYDVR0jBBgwFoAUEIXXWZ9kRxaOm8sNF/qw/SrZQ3YwDwYDVR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMCAgQwCgYIKoZIzj0EAwIDaAAwZQIwQy2eqDzxuAk0MPq29NHvHYc2wFtxjnz/KlKAvsGdvvf90+76EoRkrB/2xZ/UTRoWAjEAjP1ZeWNGo+CwN8BYfCJD5CDdFn281Y72tqy41YrhSOkFjNPj0vvzKVHLnpcsJO1j",
                r"MIICszCCAlqgAwIBAgIBATAKBggqhkjOPQQDAjA5MQwwCgYDVQQMDANURUUxKTAnBgNVBAUTIDE2Y2EyMTU0N2E5ODBkNWQ0YmZhMjZlMmJlOWI1YmQyMB4XDTcwMDEwMTAwMDAwMFoXDTQ4MDEwMTAwMDAwMFowHzEdMBsGA1UEAxMUQW5kcm9pZCBLZXlzdG9yZSBLZXkwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAAQjwR+l1Uw6PWom4EwNvL71LXobDb7THtR7TiZ+MtKqptj8FXeON+CjrcPzoli00tJRnFooRnX2Al91PbwtDfbco4IBazCCAWcwDgYDVR0PAQH/BAQDAgOIMIIBUwYKKwYBBAHWeQIBEQSCAUMwggE/AgFkCgEBAgFkCgEBBCBYOwq7RD9fxio6z2G3hGE0H2lsUNjwSLUcrBr9Ez/2rAQAMGa/hT0IAgYBhn6poBS/hUVWBFQwUjEsMCoEJWNvbS5hY3VyYXN0LmF0dGVzdGVkLmV4ZWN1dG9yLnRlc3RuZXQCAQ8xIgQg7HDCpOByoPWGVSpoNXsjaXydRfHhJXqMTSmiWsSYJDMwgaShCDEGAgECAgEGogMCAQOjBAICAQClBTEDAgEAqgMCAQG/g3cCBQC/hT4DAgEAv4VATDBKBCCaxBdBU9ReRUWw9J4i/mMnOZm2rBy2lJw6nwPsiAfu6QEB/woBAAQgCiDlskJ4BZ1xowHM2QhVorWJxxeyAiaZq8YNkuoHWua/hUEFAgMB1MC/hUIFAgMDFd6/hU4GAgQBNIq5v4VPBgIEATSKuTAKBggqhkjOPQQDAgNHADBEAiBynaYitSHopPHrJX+wVmphiLMMORl/DTVs+mvzgx52ygIgPvlbfihJQZHTcFyhiXl1DQJlwIK5fhomzCwhws8qWrI=",
            ],
            vec![
                PIXEL_ROOT_CERT,
                PIXEL_INTERMEDIATE_2_CERT,
                PIXEL_INTERMEDIATE_1_CERT,
                PIXEL_KEY_CERT,
            ],
            vec![
                r"MIIFHDCCAwSgAwIBAgIJANUP8luj8tazMA0GCSqGSIb3DQEBCwUAMBsxGTAXBgNVBAUTEGY5MjAwOWU4NTNiNmIwNDUwHhcNMTkxMTIyMjAzNzU4WhcNMzQxMTE4MjAzNzU4WjAbMRkwFwYDVQQFExBmOTIwMDllODUzYjZiMDQ1MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAr7bHgiuxpwHsK7Qui8xUFmOr75gvMsd/dTEDDJdSSxtf6An7xyqpRR90PL2abxM1dEqlXnf2tqw1Ne4Xwl5jlRfdnJLmN0pTy/4lj4/7tv0Sk3iiKkypnEUtR6WfMgH0QZfKHM1+di+y9TFRtv6y//0rb+T+W8a9nsNL/ggjnar86461qO0rOs2cXjp3kOG1FEJ5MVmFmBGtnrKpa73XpXyTqRxB/M0n1n/W9nGqC4FSYa04T6N5RIZGBN2z2MT5IKGbFlbC8UrW0DxW7AYImQQcHtGl/m00QLVWutHQoVJYnFPlXTcHYvASLu+RhhsbDmxMgJJ0mcDpvsC4PjvB+TxywElgS70vE0XmLD+OJtvsBslHZvPBKCOdT0MS+tgSOIfga+z1Z1g7+DVagf7quvmag8jfPioyKvxnK/EgsTUVi2ghzq8wm27ud/mIM7AY2qEORR8Go3TVB4HzWQgpZrt3i5MIlCaY504LzSRiigHCzAPlHws+W0rB5N+er5/2pJKnfBSDiCiFAVtCLOZ7gLiMm0jhO2B6tUXHI/+MRPjy02i59lINMRRev56GKtcd9qO/0kUJWdZTdA2XoS82ixPvZtXQpUpuL12ab+9EaDK8Z4RHJYYfCT3Q5vNAXaiWQ+8PTWm2QgBR/bkwSWc+NpUFgNPN9PvQi8WEg5UmAGMCAwEAAaNjMGEwHQYDVR0OBBYEFDZh4QB8iAUJUYtEbEf/GkzJ6k8SMB8GA1UdIwQYMBaAFDZh4QB8iAUJUYtEbEf/GkzJ6k8SMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgIEMA0GCSqGSIb3DQEBCwUAA4ICAQBOMaBc8oumXb2voc7XCWnuXKhBBK3e2KMGz39t7lA3XXRe2ZLLAkLM5y3J7tURkf5a1SutfdOyXAmeE6SRo83Uh6WszodmMkxK5GM4JGrnt4pBisu5igXEydaW7qq2CdC6DOGjG+mEkN8/TA6p3cnoL/sPyz6evdjLlSeJ8rFBH6xWyIZCbrcpYEJzXaUOEaxxXxgYz5/cTiVKN2M1G2okQBUIYSY6bjEL4aUN5cfo7ogP3UvliEo3Eo0YgwuzR2v0KR6C1cZqZJSTnghIC/vAD32KdNQ+c3N+vl2OTsUVMC1GiWkngNx1OO1+kXW+YTnnTUOtOIswUP/Vqd5SYgAImMAfY8U9/iIgkQj6T2W6FsScy94IN9fFhE1UtzmLoBIuUFsVXJMTz+Jucth+IqoWFua9v1R93/k98p41pjtFX+H8DslVgfP097vju4KDlqN64xV1grw3ZLl4CiOe/A91oeLm2UHOq6wn3esB4r2EIQKb6jTVGu5sYCcdWpXr0AUVqcABPdgL+H7qJguBw09ojm6xNIrw2OocrDKsudk/okr/AwqEyPKw9WnMlQgLIKw1rODG2NvU9oR3GVGdMkUBZutL8VuFkERQGt6vQ2OCw0sV47VMkuYbacK/xyZFiRcrPJPb41zgbQj9XAEyLKCHex0SdDrx+tWUDqG8At2JHA==",
                r"MIIDgDCCAWigAwIBAgIKA4gmZ2BliZaGDTANBgkqhkiG9w0BAQsFADAbMRkwFwYDVQQFExBmOTIwMDllODUzYjZiMDQ1MB4XDTIyMDEyNjIyNDc1MloXDTM3MDEyMjIyNDc1MlowKTETMBEGA1UEChMKR29vZ2xlIExMQzESMBAGA1UEAxMJRHJvaWQgQ0EyMHYwEAYHKoZIzj0CAQYFK4EEACIDYgAEuppxbZvJgwNXXe6qQKidXqUt1ooT8M6Q+ysWIwpduM2EalST8v/Cy2JN10aqTfUSThJha/oCtG+F9TUUviOch6RahrpjVyBdhopM9MFDlCfkiCkPCPGu2ODMj7O/bKnko2YwZDAdBgNVHQ4EFgQUu/g2rYmubOLlnpTw1bLX0nrkfEEwHwYDVR0jBBgwFoAUNmHhAHyIBQlRi0RsR/8aTMnqTxIwEgYDVR0TAQH/BAgwBgEB/wIBAjAOBgNVHQ8BAf8EBAMCAQYwDQYJKoZIhvcNAQELBQADggIBAIFxUiFHYfObqrJM0eeXI+kZFT57wBplhq+TEjd+78nIWbKvKGUFlvt7IuXHzZ7YJdtSDs7lFtCsxXdrWEmLckxRDCRcth3Eb1leFespS35NAOd0Hekg8vy2G31OWAe567l6NdLjqytukcF4KAzHIRxoFivN+tlkEJmg7EQw9D2wPq4KpBtug4oJE53R9bLCT5wSVj63hlzEY3hC0NoSAtp0kdthow86UFVzLqxEjR2B1MPCMlyIfoGyBgkyAWhd2gWN6pVeQ8RZoO5gfPmQuCsn8m9kv/dclFMWLaOawgS4kyAn9iRi2yYjEAI0VVi7u3XDgBVnowtYAn4gma5q4BdXgbWbUTaMVVVZsepXKUpDpKzEfss6Iw0zx2Gql75zRDsgyuDyNUDzutvDMw8mgJmFkWjlkqkVM2diDZydzmgi8br2sJTLdG4lUwvedIaLgjnIDEG1J8/5xcPVQJFgRf3m5XEZB4hjG3We/49p+JRVQSpE1+QzG0raYpdNsxBUO+41diQo7qC7S8w2J+TMeGdpKGjCIzKjUDAy2+gOmZdZacanFN/03SydbKVHV0b/NYRWMa4VaZbomKON38IH2ep8pdj++nmSIXeWpQE8LnMEdnUFjvDzp0f0ELSXVW2+5xbl+fcqWgmOupmU4+bxNJLtknLo49Bg5w9jNn7T7rkF",
                r"MIIB1jCCAVygAwIBAgITcM+MOFG89A40Y9HIF2M9NY+PTzAKBggqhkjOPQQDAzApMRMwEQYDVQQKEwpHb29nbGUgTExDMRIwEAYDVQQDEwlEcm9pZCBDQTIwHhcNMjMxMDE1MTkyNDUxWhcNMjMxMjA0MTkyNDUwWjApMRMwEQYDVQQKEwpHb29nbGUgTExDMRIwEAYDVQQDEwlEcm9pZCBDQTMwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAARr95AOrHbuQ6kHxl+7t71D6qJmN8YreftJFZBhPA5Kw0RrM6KuYvYBF6gPqt8Sov+UWvWtDeL5sGGRybmN/gtWo2MwYTAOBgNVHQ8BAf8EBAMCAgQwDwYDVR0TAQH/BAUwAwEB/zAdBgNVHQ4EFgQUSDkbhn725ZTB2PKmrq+6FSkfWmowHwYDVR0jBBgwFoAUu/g2rYmubOLlnpTw1bLX0nrkfEEwCgYIKoZIzj0EAwMDaAAwZQIxALFBTZe5vl4PBmAO8KHACvb4Qg6TLsnTrunrs9eja1oMmeYPO1dU0V8N0+nZRkP59gIwXDIumMPdaqz39DW6g38vizFhdvimGaEDQBZn7irtlbV5mao04wTZ3WRgp2L9fNHP",
                r"MIIB3jCCAYSgAwIBAgIRAPb+T4MpCZjDmbqiVhfq+HIwCgYIKoZIzj0EAwIwKTETMBEGA1UEChMKR29vZ2xlIExMQzESMBAGA1UEAxMJRHJvaWQgQ0EzMB4XDTIzMTAxNDE1MzYxMFoXDTIzMTExODE1MzYxMFowPzESMBAGA1UEChMJU3Ryb25nQm94MSkwJwYDVQQDEyBmNmZlNGY4MzI5MDk5OGMzOTliYWEyNTYxN2VhZjg3MjBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABCUqltkFs/h7vHKQ2ZCcmw3vKi7AgFZZ24+lrL6jxXJ6l7HslskshJDa3QlYCAl6s/EBw94qilUvTI2h/V/h3W+jdzB1MB0GA1UdDgQWBBQIfrJVw4FkRZjl3aykqPe67idvPTAfBgNVHSMEGDAWgBRIORuGfvbllMHY8qaur7oVKR9aajAPBgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwICBDASBgorBgEEAdZ5AgEeBAShARhAMAoGCCqGSM49BAMCA0gAMEUCIBmpMpCMKbBuvMK+0FHR5pnjJWfmpwOt9T8FgedpEzdWAiEA8gzScTX2ZUc5DMYm2Zrhx4SRMVP4XNHGTIhiSjWvM5w=",
                r"MIICvTCCAmSgAwIBAgIBATAKBggqhkjOPQQDAjA/MRIwEAYDVQQKEwlTdHJvbmdCb3gxKTAnBgNVBAMTIGY2ZmU0ZjgzMjkwOTk4YzM5OWJhYTI1NjE3ZWFmODcyMB4XDTcwMDEwMTAwMDAwMFoXDTQ4MDEwMTAwMDAwMFowHzEdMBsGA1UEAxMUQW5kcm9pZCBLZXlzdG9yZSBLZXkwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAATDGqojP3482hCBZkLBXCfKeaq43Xh+AeUOI9Ym9F0ch3gtfgfJmjU2Z3EXR7YYE+AKh91ysxVc0Xs7lpm2HOcGo4IBbzCCAWswDgYDVR0PAQH/BAQDAgMIMIIBVwYKKwYBBAHWeQIBEQSCAUcwggFDAgIBLAoBAgICASwKAQIEIFn60RrhIWvgMbYX5Yen3U+9/33bdmX070ih8hd+Z4I/BAAwZb+FPQgCBgGLOSJgrr+FRVUEUzBRMSswKQQkY29tLmFjdXJhc3QuYXR0ZXN0ZWQuZXhlY3V0b3IuY2FuYXJ5AgEPMSIEIOxwwqTgcqD1hlUqaDV7I2l8nUXx4SV6jE0polrEmCQzMIGnoQsxCQIBAgIBAwIBBqIDAgEDowQCAgEApQUxAwIBAKoDAgEBv4N3AgUAv4U+AwIBAL+FQEwwSgQgJqxMYL6x43g1fK0MMGE0evjfb7q7sNjOokRYVe4B42gBAf8KAQAEIHkY01pAyfdaFbiuYRh2UFMeF4ltt8FCbyvwoDT6CfTLv4VBBQIDAiLgv4VCBQIDAxZGv4VOBgIEATSzXb+FTwYCBAE0s10wCgYIKoZIzj0EAwIDRwAwRAIgP/JcTQhcHftUQRZSUdKedvMAUzj02tfvKP8t/7ruzgcCIHMS0aiL/dbzq+E1+bxtPL0pMZPjCxyGLdPcIySI8yk/",
            ],
        ];

        for chain in chains {
            let decoded_chain = decode_certificate_chain(&chain);
            let (_, cert, _) =
                validate_certificate_chain(&decoded_chain).expect("validating chain failed");
            let key_description = extract_attestation(cert.extensions).map_err(|err| {
                dbg!(err.clone());

                err
            })?;
            match &key_description {
                KeyDescription::V4(key_description) => {
                    assert_eq!(key_description.attestation_version, 4)
                }
                KeyDescription::V100(key_description) => {
                    assert_eq!(key_description.attestation_version, 100)
                }
                KeyDescription::V200(key_description) => {
                    assert_eq!(key_description.attestation_version, 200)
                }
                KeyDescription::V300(key_description) => {
                    assert_eq!(key_description.attestation_version, 300)
                }
                _ => return Err(()),
            }
            let _: BoundedKeyDescription = key_description.try_into()?;
        }

        Ok(())
    }

    #[test]
    fn test_validate_pixel_invalid_signature_chain() -> Result<(), ()> {
        let chain = vec![
            PIXEL_ROOT_CERT,
            PIXEL_INTERMEDIATE_2_CERT,
            PIXEL_INTERMEDIATE_1_CERT,
            PIXEL_KEY_CERT_INVALID,
        ];
        let decoded_chain = decode_certificate_chain(&chain);
        let res = validate_certificate_chain(&decoded_chain);
        match res {
            Err(e) => assert_eq!(e, ValidationError::InvalidSignature),
            _ => return Err(()),
        };
        Ok(())
    }

    #[test]
    fn test_validate_pixel_untrusted_root_chain() -> Result<(), ()> {
        let chain = vec![
            PIXEL_ROOT_CERT_UNTRUSTED,
            PIXEL_INTERMEDIATE_2_CERT,
            PIXEL_INTERMEDIATE_1_CERT,
            PIXEL_KEY_CERT_INVALID,
        ];
        let decoded_chain = decode_certificate_chain(&chain);
        let res = validate_certificate_chain(&decoded_chain);
        match res {
            Err(e) => assert_eq!(e, ValidationError::InvalidSignature),
            _ => return Err(()),
        };
        Ok(())
    }
}
