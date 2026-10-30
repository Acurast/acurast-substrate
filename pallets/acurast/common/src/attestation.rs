pub mod asn;
pub mod error;

use crate::AttestationSecurityLevel;
use asn::*;
use asn1::{oid, BitString, Enumerated, ObjectIdentifier, ParseError, SequenceOf};
use ecdsa_vendored::hazmat::VerifyPrimitive;
use error::ValidationError;
use frame_support::{traits::ConstU32, BoundedVec};
use num_bigint::BigUint;
use p256::ecdsa::{signature::Verifier, VerifyingKey};
use p384::elliptic_curve::{
	generic_array::{typenum::Unsigned, GenericArray},
	sec1::{FromEncodedPoint, ModulusSize},
};

use sha2::Digest;
use sp_std::prelude::*;

pub const CHAIN_MAX_LENGTH: u32 = 7;
pub const CERT_MAX_LENGTH: u32 = 3000;
pub type CertificateInput = BoundedVec<u8, ConstU32<CERT_MAX_LENGTH>>;
pub type CertificateChainInput = BoundedVec<CertificateInput, ConstU32<CHAIN_MAX_LENGTH>>;

pub fn parse_cert(serialized: &[u8]) -> Result<Certificate<'_>, ParseError> {
	let data = asn1::parse_single::<Certificate>(serialized)?;
	Ok(data)
}

/// A certificate of a chain with the DER of its signed part.
struct ChainCertificate<'a> {
	cert: Certificate<'a>,
	payload: &'a [u8],
}

/// Parses each certificate of the chain once, for [verify_chain] and [ensure_chain_policy].
fn parse_certificate_chain(
	chain: &CertificateChainInput,
) -> Result<Vec<ChainCertificate<'_>>, ValidationError> {
	chain
		.iter()
		.map(|serialized| {
			let raw = asn1::parse_single::<CertificateRawPayload>(serialized)?;
			Ok(ChainCertificate {
				cert: Certificate {
					tbs_certificate: raw.tbs_certificate.parse()?,
					signature_algorithm: raw.signature_algorithm,
					signature_value: raw.signature_value,
				},
				payload: raw.tbs_certificate.full_data(),
			})
		})
		.collect()
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
const APPLE_DEVICE_ATTESTATION_KEY_USAGE_PROPERTIES: ObjectIdentifier =
	oid!(1, 2, 840, 113635, 100, 8, 5);
const APPLE_DEVICE_ATTESTATION_DEVICE_OS_INFORMATION: ObjectIdentifier =
	oid!(1, 2, 840, 113635, 100, 8, 7);
const APPLE_DEVICE_ATTESTATION_NONCE: ObjectIdentifier = oid!(1, 2, 840, 113635, 100, 8, 2);

/// Extracts and parses the attestation from the extension field of a X.509 certificate.
pub fn extract_attestation<'a>(
	extensions: &Option<SequenceOf<'a, Extension<'a>>>,
) -> Result<ParsedAttestation<'a>, ValidationError> {
	let extensions =
		extensions.clone().ok_or(ValidationError::ExtensionMissing)?.collect::<Vec<_>>();
	if let Some(extension) = &extensions.iter().find(|e| e.extn_id == KEY_ATTESTATION_OID) {
		return Ok(ParsedAttestation::KeyDescription(parse_key_description(extension)?));
	}

	Ok(ParsedAttestation::DeviceAttestation(parse_apple_attestation(&extensions)?))
}

fn parse_key_description<'a>(
	extension: &Extension<'a>,
) -> Result<KeyDescription<'a>, ValidationError> {
	Ok(asn1::parse_single::<KeyDescription>(extension.extn_value)?)
}

fn parse_apple_attestation<'a>(
	extensions: &[Extension<'a>],
) -> Result<DeviceAttestation<'a>, ValidationError> {
	if let Some(key_usage_properties) = &extensions
		.iter()
		.find(|e| e.extn_id == APPLE_DEVICE_ATTESTATION_KEY_USAGE_PROPERTIES)
	{
		let key_usage_properties = asn1::parse_single::<DeviceAttestationKeyUsageProperties>(
			key_usage_properties.extn_value,
		)?;
		if let Some(device_os_information) = &extensions
			.iter()
			.find(|e| e.extn_id == APPLE_DEVICE_ATTESTATION_DEVICE_OS_INFORMATION)
		{
			let device_os_information = asn1::parse_single::<DeviceAttestationDeviceOSInformation>(
				device_os_information.extn_value,
			)?;
			if let Some(nonce) =
				&extensions.iter().find(|e| e.extn_id == APPLE_DEVICE_ATTESTATION_NONCE)
			{
				let nonce = asn1::parse_single::<DeviceAttestationNonce>(nonce.extn_value)?;
				return Ok(DeviceAttestation {
					key_usage_properties,
					device_os_information,
					nonce,
				});
			}
		}
	}
	Err(ValidationError::ExtensionMissing)
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
		match info.algorithm.algorithm {
			RSA_PBK => {
				let pbk = parse_rsa_pbk(info.subject_public_key.as_bytes())?;
				Ok(PublicKey::RSA(pbk))
			},
			ECDSA_PBK => {
				let pbk_param =
					info.algorithm.parameters.ok_or(ValidationError::MissingECDSAAlgorithmTyp)?;
				let typ = asn1::parse_single::<ObjectIdentifier>(pbk_param.full_data())?;
				match typ {
					CURVE_P256 => {
						let verifying_key =
							VerifyingKey::from_sec1_bytes(info.subject_public_key.as_bytes())
								.or(Err(ValidationError::ParseP256PublicKey))?;
						Ok(PublicKey::ECDSA(ECDSACurve::CurveP256(verifying_key)))
					},
					CURVE_P384 => {
						// Only accept the SEC 1 encodings that carry both coordinates: uncompressed
						// (0x04) and hybrid (0x06/0x07), each followed by a full `x || y` coordinate
						// pair. Compressed or otherwise malformed points are rejected explicitly
						// instead of ignoring the leading byte; this also guards against an
						// out-of-bounds slice on truncated keys.
						let encoded = info.subject_public_key.as_bytes();
						let (tag, coordinates) = match encoded.split_first() {
							Some((
								tag @ &(SEC1_TAG_UNCOMPRESSED
								| SEC1_TAG_HYBRID_EVEN_Y
								| SEC1_TAG_HYBRID_ODD_Y),
								coordinates,
							)) if coordinates.len() == P384_UNTAGGED_POINT_SIZE => (*tag, coordinates),
							_ => return Err(ValidationError::ParseP384PublicKey),
						};
						let (x, y) = coordinates.split_at(P384_COORDINATE_SIZE);
						// A hybrid tag repeats the parity of y; require the two to agree so that a
						// point keeps a single accepted encoding per form.
						let y_is_odd = y[P384_COORDINATE_SIZE - 1] & 1 == 1;
						let tag_agrees_with_y = match tag {
							SEC1_TAG_HYBRID_EVEN_Y => !y_is_odd,
							SEC1_TAG_HYBRID_ODD_Y => y_is_odd,
							// the uncompressed tag carries no parity
							_ => true,
						};
						if !tag_agrees_with_y {
							return Err(ValidationError::ParseP384PublicKey);
						}
						let encoded_point = p384::EncodedPoint::from_affine_coordinates(
							GenericArray::from_slice(x),
							GenericArray::from_slice(y),
							false,
						);
						let point = Option::<p384::AffinePoint>::from(
							p384::AffinePoint::from_encoded_point(&encoded_point),
						)
						.ok_or(ValidationError::ParseP384PublicKey)?;
						Ok(PublicKey::ECDSA(ECDSACurve::CurveP384(point)))
					},
					_ => Result::Err(ValidationError::UnsupportedSignatureAlgorithm)?,
				}
			},
			_ => Result::Err(ValidationError::UnsupportedPublicKeyAlgorithm),
		}
	}
}

const CURVE_P256: ObjectIdentifier = oid!(1, 2, 840, 10045, 3, 1, 7);
const CURVE_P384: ObjectIdentifier = oid!(1, 3, 132, 0, 34);

/// Size of the P-384 base field, as used for the curve's serialized field elements.
type P384FieldSize = p384::elliptic_curve::FieldSize<p384::NistP384>;
/// Length in bytes of one serialized P-384 point coordinate.
const P384_COORDINATE_SIZE: usize = <P384FieldSize as Unsigned>::USIZE;
/// Length in bytes of an untagged P-384 point, i.e. the `x || y` coordinates without a SEC 1 tag.
const P384_UNTAGGED_POINT_SIZE: usize =
	<<P384FieldSize as ModulusSize>::UntaggedPointSize as Unsigned>::USIZE;
/// SEC 1 tag of an uncompressed point, followed by `x || y`.
const SEC1_TAG_UNCOMPRESSED: u8 = 0x04;
/// SEC 1 tag of a hybrid point with an even y coordinate, followed by `x || y`.
const SEC1_TAG_HYBRID_EVEN_Y: u8 = 0x06;
/// SEC 1 tag of a hybrid point with an odd y coordinate, followed by `x || y`.
const SEC1_TAG_HYBRID_ODD_Y: u8 = 0x07;

/// OID of the X.509v3 BasicConstraints extension.
const BASIC_CONSTRAINTS_OID: ObjectIdentifier = oid!(2, 5, 29, 19);
/// OID of the X.509v3 KeyUsage extension.
const KEY_USAGE_OID: ObjectIdentifier = oid!(2, 5, 29, 15);
/// Bit position of `keyCertSign` in the KeyUsage bit string.
/// [See RFC](https://www.rfc-editor.org/rfc/rfc5280#section-4.2.1.3)
const KEY_CERT_SIGN_BIT: usize = 5;

/// Enforces that a certificate used to sign another certificate in the chain is a
/// legitimate CA, as required by [RFC 5280](https://www.rfc-editor.org/rfc/rfc5280):
///
/// - the BasicConstraints extension is present with `cA = TRUE`,
/// - the pathLenConstraint (if present) is not exceeded by the number of intermediate
///   CA certificates that still follow this one in the chain, and
/// - the KeyUsage extension is present with the `keyCertSign` bit set.
///
/// Without these checks any leaf certificate (e.g. an attestation key) could be used to
/// mint a forged sub-certificate that the chain validation would otherwise accept.
fn ensure_ca_certificate(
	cert: &Certificate<'_>,
	following_intermediate_ca_count: u64,
) -> Result<(), ValidationError> {
	let extensions = cert
		.tbs_certificate
		.extensions
		.clone()
		.ok_or(ValidationError::ExtensionMissing)?
		.collect::<Vec<_>>();

	let basic_constraints = extensions
		.iter()
		.find(|e| e.extn_id == BASIC_CONSTRAINTS_OID)
		.ok_or(ValidationError::BasicConstraintsMissing)?;
	let basic_constraints = asn1::parse_single::<BasicConstraints>(basic_constraints.extn_value)?;
	if !basic_constraints.ca {
		return Err(ValidationError::NotACertificateAuthority);
	}
	if let Some(max) = basic_constraints.path_len_constraint {
		if following_intermediate_ca_count > max {
			return Err(ValidationError::PathLenConstraintViolated);
		}
	}

	let key_usage = extensions
		.iter()
		.find(|e| e.extn_id == KEY_USAGE_OID)
		.ok_or(ValidationError::KeyUsageMissing)?;
	let key_usage = asn1::parse_single::<BitString>(key_usage.extn_value)?;
	if !key_usage.has_bit_set(KEY_CERT_SIGN_BIT) {
		return Err(ValidationError::KeyCertSignNotAllowed);
	}

	Ok(())
}

/// Whether the certificate carries an Android key attestation, RKP provisioning info or Apple device attestation
/// extension, which a non-CA issuer of the leaf must not, as such a device key could forge a sub-certificate.
fn carries_attestation(cert: &Certificate<'_>) -> bool {
	cert.tbs_certificate.extensions.clone().into_iter().flatten().any(|e| {
		[
			KEY_ATTESTATION_OID,
			PROVISIONING_INFO_OID,
			APPLE_DEVICE_ATTESTATION_KEY_USAGE_PROPERTIES,
			APPLE_DEVICE_ATTESTATION_DEVICE_OS_INFORMATION,
			APPLE_DEVICE_ATTESTATION_NONCE,
		]
		.contains(&e.extn_id)
	})
}

fn validate(
	cert: &Certificate<'_>,
	payload: &[u8],
	pbk: &PublicKey,
) -> Result<(), ValidationError> {
	if cert.signature_algorithm.algorithm != cert.tbs_certificate.signature.algorithm {
		return Err(ValidationError::SignatureMismatch);
	}
	match cert.signature_algorithm.algorithm {
		RSA_ALGORITHM => match pbk {
			PublicKey::RSA(pbk) => validate_rsa(payload, &cert.signature_value, pbk),
			_ => Err(ValidationError::UnsupportedPublicKeyAlgorithm),
		},
		ECDSA_WITH_SHA256_ALGORITHM => match pbk {
			PublicKey::ECDSA(pbk) => {
				validate_ecdsa::<sha2::Sha256>(payload, &cert.signature_value, pbk)
			},
			_ => Err(ValidationError::UnsupportedPublicKeyAlgorithm),
		},
		ECDSA_WITH_SHA384_ALGORITHM => match pbk {
			PublicKey::ECDSA(pbk) => {
				validate_ecdsa::<sha2::Sha384>(payload, &cert.signature_value, pbk)
			},
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
	use rsa::signature::Verifier;

	let modulus = rsa::BigUint::from_bytes_be(pbk.modulus.to_bytes_be().as_slice());
	let exponent = rsa::BigUint::from_bytes_be(pbk.exponent.to_bytes_be().as_slice());
	let pk =
		rsa::RsaPublicKey::new(modulus, exponent).map_err(|_| ValidationError::InvalidSignature)?;
	let verifying_key = rsa::pkcs1v15::VerifyingKey::<sha2::Sha256>::new(pk);
	let signature = rsa::pkcs1v15::Signature::try_from(signature.as_bytes())
		.map_err(|_| ValidationError::InvalidSignature)?;
	verifying_key
		.verify(payload, &signature)
		.map_err(|_| ValidationError::InvalidSignature)?;

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
			let signature = p256::ecdsa::Signature::from_der(signature.as_bytes())
				.or(Err(ValidationError::InvalidSignatureEncoding))?;
			verifying_key
				.verify(payload, &signature)
				.or(Err(ValidationError::InvalidSignature))?;
		},
		ECDSACurve::CurveP384(affine_point) => {
			let signature = ecdsa_vendored::Signature::from_der(signature.as_bytes())
				.or(Err(ValidationError::InvalidSignatureEncoding))?;

			let hashed = &D::digest(payload);
			let mut padded: [u8; 48] = [0; 48];
			// Left-pad shorter digests (e.g. SHA-256) into the 48-byte P-384 field width.
			// Reject unexpected digest sizes instead of panicking in `copy_from_slice`.
			match hashed.len() {
				32 => padded[16..].copy_from_slice(hashed),
				48 => padded.copy_from_slice(hashed),
				_ => return Err(ValidationError::InvalidSignatureEncoding),
			}
			let payload = p384::FieldBytes::from_slice(&padded);

			affine_point
				.verify_prehashed(*payload, &signature)
				.or(Err(ValidationError::InvalidSignature))?;
		},
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

/// A chain accepted by [validate_certificate_chain].
pub struct ValidatedChain<'a> {
	pub cert_ids: Vec<CertificateId>,
	/// The validity of the leaf certificate.
	pub validity: Validity,
	pub public_key: PublicKey,
	/// Whether it is an RKP chain, whose leaf's issuer is the RKP device certificate.
	pub is_rkp: bool,
	pub attestation: ParsedAttestation<'a>,
}

/// Validates the chain at `now` (unix millis): its signatures, issuers and validity down from a trusted root and, for
/// Android, the shape of a genuine keybox or RKP chain with accepted provisioning info.
pub fn validate_certificate_chain(
	chain: &CertificateChainInput,
	now: u64,
) -> Result<ValidatedChain<'_>, ValidationError> {
	let certs = parse_certificate_chain(chain)?;
	let VerifiedChain { cert_ids, public_key, root_included, leaf_issuer_ca, apple_root } =
		verify_chain(&certs)?;
	let leaf = &certs.last().ok_or(ValidationError::ChainTooShort)?.cert.tbs_certificate;
	let attestation =
		extract_attestation(&leaf.extensions).map_err(|_| ValidationError::ExtractAttestation)?;
	if apple_root != matches!(attestation, ParsedAttestation::DeviceAttestation(_)) {
		return Err(ValidationError::AttestationRootMismatch);
	}
	let is_rkp = ensure_chain_policy(&certs, root_included, leaf_issuer_ca, &attestation, now)?;
	Ok(ValidatedChain {
		cert_ids,
		validity: leaf.validity.clone(),
		public_key,
		is_rkp,
		attestation,
	})
}

/// What [verify_chain] found, for [ensure_chain_policy].
struct VerifiedChain {
	cert_ids: Vec<CertificateId>,
	public_key: PublicKey,
	/// Whether the chain starts with a trusted root.
	root_included: bool,
	/// Whether the leaf's issuer is a CA; a non-CA one without attestation extension is let through.
	leaf_issuer_ca: bool,
	/// Whether the chain verifies against the Apple root rather than a Google one.
	apple_root: bool,
}

/// Verifies the signatures down from a trusted root, which may only head the chain, and that every issuer is a CA
/// except a leaf issuer without attestation extension, which [ensure_chain_policy] only allows in keybox chains.
fn verify_chain(chain: &[ChainCertificate<'_>]) -> Result<VerifiedChain, ValidationError> {
	let trusted_roots = trusted_roots()?;
	let mut cert_ids = Vec::<CertificateId>::new();
	let mut prev_pbk = Option::<PublicKey>::None;
	let mut head = None;
	let mut leaf_issuer_ca = true;
	for (index, ChainCertificate { cert, payload }) in chain.iter().enumerate() {
		let current_pbk = PublicKey::parse(&cert.tbs_certificate.subject_public_key_info)?;
		let root = trusted_roots.iter().position(|root| *root == current_pbk);
		match &prev_pbk {
			Some(prev_pbk) => verify_signed_by(cert, payload, prev_pbk, root)?,
			None => {
				let head_root =
					verify_chain_head(cert, payload, &current_pbk, root, &trusted_roots)?;
				head = Some((head_root, root.is_some()));
			},
		}
		// Every certificate except the last (the end-entity/leaf, e.g. the attestation
		// key) signs the next certificate in the chain, so it must be a valid CA.
		// Skipping this lets a leaf certificate forge a sub-certificate.
		if index + 1 < chain.len() {
			let following_intermediate_ca_count = (chain.len() - 2 - index) as u64;
			leaf_issuer_ca &= ensure_issuer(cert, following_intermediate_ca_count)?;
		}
		cert_ids
			.push(unique_id(&cert.tbs_certificate.issuer, &cert.tbs_certificate.serial_number)?);
		// it's crucial for security to pass on a non-null public key here,
		// otherwise self-signed certificates would get accepted later down the chain
		prev_pbk = Some(current_pbk);
	}
	let (Some((root, root_included)), Some(public_key)) = (head, prev_pbk) else {
		return Err(ValidationError::ChainTooShort);
	};
	Ok(VerifiedChain {
		cert_ids,
		public_key,
		root_included,
		leaf_issuer_ca,
		apple_root: root == APPLE_ROOT,
	})
}

/// Verifies that `cert`, which does not head the chain, is signed by `issuer_key` and is not itself the trusted root at
/// position `root`.
fn verify_signed_by(
	cert: &Certificate<'_>,
	payload: &[u8],
	issuer_key: &PublicKey,
	root: Option<usize>,
) -> Result<(), ValidationError> {
	if root.is_some() {
		return Err(ValidationError::UnexpectedChainShape);
	}
	validate(cert, payload, issuer_key)
}

/// Verifies the first certificate against itself if it is the trusted root at position `root`, or else against the
/// trusted root that signed it. Returns that root's position in `trusted_roots`.
fn verify_chain_head(
	cert: &Certificate<'_>,
	payload: &[u8],
	public_key: &PublicKey,
	root: Option<usize>,
	trusted_roots: &[PublicKey],
) -> Result<usize, ValidationError> {
	if let Some(root) = root {
		validate(cert, payload, public_key)?;
		return Ok(root);
	}
	// this can happen if the submitted certificate chain does not contain the root,
	// which is fine, we can start validating from the intermediate certificate since
	// we already have the root public key.
	trusted_roots
		.iter()
		.position(|root| validate(cert, payload, root).is_ok())
		.ok_or(ValidationError::InvalidSignature)
}

/// Ensures `cert`, which signs the next certificate, is a CA, except that the leaf's issuer may be a non-CA without
/// attestation extension. Returns whether it is a CA.
fn ensure_issuer(
	cert: &Certificate<'_>,
	following_intermediate_ca_count: u64,
) -> Result<bool, ValidationError> {
	match ensure_ca_certificate(cert, following_intermediate_ca_count) {
		Ok(()) => Ok(true),
		Err(
			ValidationError::BasicConstraintsMissing | ValidationError::NotACertificateAuthority,
		) if following_intermediate_ca_count == 0 && !carries_attestation(cert) => Ok(false),
		Err(error) => Err(error),
	}
}

/// Position of the Apple root in [trusted_roots].
const APPLE_ROOT: usize = 1;

fn trusted_roots() -> Result<[PublicKey; 3], ValidationError> {
	let parse = |key| PublicKey::parse(&asn1::parse_single::<SubjectPublicKeyInfo>(key)?);
	// App Attest chains never include their root, so try Apple before the Google P-384 root
	Ok([parse(GOOGLE_ROOT_PUB_KEY)?, parse(APPLE_ROOT_PUB_KEY)?, parse(GOOGLE_P384_ROOT_PUB_KEY)?])
}

/// OID of the RKP provisioning info extension carried by remotely provisioned attestation certificates.
const PROVISIONING_INFO_OID: ObjectIdentifier = oid!(1, 3, 6, 1, 4, 1, 11129, 2, 1, 30);
/// Lowest bucketed count of certificates issued to one device in 30 days that is rejected.
const MAX_CERTIFICATES_ISSUED: u64 = 1024;
const KEYBOX_CHAIN_LENGTH: usize = 4;
const SERIAL_NUMBER_OID: ObjectIdentifier = oid!(2, 5, 4, 5);
const ORGANIZATION_OID: ObjectIdentifier = oid!(2, 5, 4, 10);

/// The raw value of the first attribute of type `typ` in `name`.
fn attribute<'a>(name: &Name<'a>, typ: &ObjectIdentifier) -> Option<&'a [u8]> {
	attributes(name).find(|a| a.typ == *typ).map(|a| a.value.data())
}

/// The attributes of all relative distinguished names of `name`.
fn attributes<'a>(name: &Name<'a>) -> impl Iterator<Item = AttributeTypeAndValue<'a>> {
	let Name::RDNSequence(rdns) = name.clone();
	rdns.flatten()
}

/// Whether any attribute value of `name` mentions StrongBox, in any case.
fn mentions_strongbox(name: &Name<'_>) -> bool {
	attributes(name)
		.any(|a| a.value.data().windows(9).any(|w| w.eq_ignore_ascii_case(b"strongbox")))
}

/// Ensures that the key's attestation and KeyMint security levels agree and are not
/// software, and that a StrongBox key comes from a chain claiming StrongBox and a TEE key from none claiming otherwise.
fn ensure_security_level(
	key_description: &KeyDescription<'_>,
	leaf_issuer: Option<&Name<'_>>,
	issued_by_root: Option<&Name<'_>>,
	is_rkp: bool,
) -> Result<(), ValidationError> {
	use AttestationSecurityLevel::{Software, StrongBox, TrustedEnvironemnt};
	let level_of =
		|level: &SecurityLevel| AttestationSecurityLevel::from(Enumerated::new(level.value()));
	let level = level_of(&key_description.key_mint_security_level);
	// an RKP chain claims the level in its device certificate's organization, a factory one only names StrongBox
	let claimed = if is_rkp {
		Some(match leaf_issuer.and_then(|name| attribute(name, &ORGANIZATION_OID)) {
			Some(b"TEE") => TrustedEnvironemnt,
			Some(b"StrongBox") => StrongBox,
			_ => Software,
		})
	} else {
		[leaf_issuer, issued_by_root]
			.into_iter()
			.flatten()
			.any(mentions_strongbox)
			.then_some(StrongBox)
	};
	let consistent = level == level_of(&key_description.attestation_security_level)
		&& matches!(level, TrustedEnvironemnt | StrongBox);
	let matches_chain = match level {
		StrongBox => claimed == Some(StrongBox),
		_ => claimed.is_none_or(|claimed| claimed == level),
	};
	if !(consistent && matches_chain) {
		return Err(ValidationError::SecurityLevelMismatch);
	}
	Ok(())
}

/// Ensures a chain checked by [verify_chain] is valid at `now` (unix millis) with a CA issuing the leaf, both waived for
/// a keybox chain's intermediates, and for Android has the shape of a genuine keybox or RKP chain with accepted
/// provisioning info. Returns whether it is an RKP chain.
fn ensure_chain_policy(
	chain: &[ChainCertificate<'_>],
	root_trusted: bool,
	leaf_issuer_ca: bool,
	attestation: &ParsedAttestation<'_>,
	now: u64,
) -> Result<bool, ValidationError> {
	let mut intermediate_expired = false;
	let mut provisioning_info = None;
	for (index, ChainCertificate { cert, .. }) in chain.iter().enumerate() {
		let tbs = &cert.tbs_certificate;
		if index > 0 || !root_trusted {
			intermediate_expired |= ensure_valid_at(&tbs.validity, index + 1 == chain.len(), now)?;
		}
		for info in provisioning_infos(tbs) {
			if provisioning_info.replace((index, info)).is_some() {
				return Err(ValidationError::UnexpectedChainShape);
			}
		}
	}
	let ParsedAttestation::KeyDescription(key_description) = attestation else {
		ensure_not_exempted(intermediate_expired, leaf_issuer_ca)?;
		return Ok(false);
	};
	ensure_leaf_serial_number(chain)?;
	let rkp_info = if is_keybox(chain, root_trusted, provisioning_info.is_some()) {
		None
	} else {
		ensure_not_exempted(intermediate_expired, leaf_issuer_ca)?;
		// an RKP chain has the provisioning info directly above the leaf; its length may change
		match provisioning_info {
			Some((index, info)) if index + 2 == chain.len() => Some(info),
			_ => return Err(ValidationError::UnexpectedChainShape),
		}
	};
	let issued_by_root = issued_by_root(chain, root_trusted);
	ensure_security_level(key_description, leaf_issuer(chain), issued_by_root, rkp_info.is_some())?;
	if let Some(info) = rkp_info {
		ensure_provisioning_info_accepted(info, verified_boot_hash(key_description))?;
	}
	Ok(rkp_info.is_some())
}

/// Ensures a certificate is valid at `now` (unix millis), letting it expire unless it is the leaf. Returns whether it
/// expired.
fn ensure_valid_at(validity: &Validity, is_leaf: bool, now: u64) -> Result<bool, ValidationError> {
	let not_yet_valid = now < validity.not_before.timestamp_millis()?;
	let expired = now >= validity.not_after.timestamp_millis()?;
	if not_yet_valid || (expired && is_leaf) {
		return Err(ValidationError::CertificateNotValidAtTime);
	}
	Ok(expired)
}

/// The values of the RKP provisioning info extensions of a certificate.
fn provisioning_infos<'a>(tbs: &TBSCertificate<'a>) -> impl Iterator<Item = &'a [u8]> {
	tbs.extensions
		.clone()
		.into_iter()
		.flatten()
		.filter(|extension| extension.extn_id == PROVISIONING_INFO_OID)
		.map(|extension| extension.extn_value)
}

/// Ensures no intermediate expired and the leaf's issuer is a CA; only factory keybox chains are exempt, as Google's
/// verifier accepts both for their unrotatable keys.
fn ensure_not_exempted(
	intermediate_expired: bool,
	leaf_issuer_ca: bool,
) -> Result<(), ValidationError> {
	if intermediate_expired {
		return Err(ValidationError::CertificateNotValidAtTime);
	}
	if !leaf_issuer_ca {
		return Err(ValidationError::NotACertificateAuthority);
	}
	Ok(())
}

/// Ensures the Android leaf certificate has the serial number 1 the secure hardware always assigns.
fn ensure_leaf_serial_number(chain: &[ChainCertificate<'_>]) -> Result<(), ValidationError> {
	if chain
		.last()
		.is_some_and(|leaf| leaf.cert.tbs_certificate.serial_number.as_bytes() != [1])
	{
		return Err(ValidationError::UnexpectedLeafSerialNumber);
	}
	Ok(())
}

/// Whether an Android chain is a factory keybox chain: four certificates with the root, the certificate the root issued
/// naming a serial number, and no RKP provisioning info.
fn is_keybox(
	chain: &[ChainCertificate<'_>],
	root_trusted: bool,
	has_provisioning_info: bool,
) -> bool {
	let length_with_root = chain.len() + usize::from(!root_trusted);
	let factory = issued_by_root(chain, root_trusted)
		.is_some_and(|name| attribute(name, &SERIAL_NUMBER_OID).is_some());
	factory && length_with_root == KEYBOX_CHAIN_LENGTH && !has_provisioning_info
}

/// The subject of the certificate the trusted root issued.
fn issued_by_root<'c, 'a>(
	chain: &'c [ChainCertificate<'a>],
	root_trusted: bool,
) -> Option<&'c Name<'a>> {
	chain
		.get(if root_trusted { 1 } else { 0 })
		.map(|c| &c.cert.tbs_certificate.subject)
}

/// The subject of the certificate that issued the leaf.
fn leaf_issuer<'c, 'a>(chain: &'c [ChainCertificate<'a>]) -> Option<&'c Name<'a>> {
	chain
		.len()
		.checked_sub(2)
		.and_then(|index| chain.get(index))
		.map(|c| &c.cert.tbs_certificate.subject)
}

/// The verified boot hash an Android attestation reports in its hardware-enforced root of trust.
fn verified_boot_hash<'a>(key_description: &KeyDescription<'a>) -> Option<&'a [u8]> {
	key_description
		.tee_enforced
		.root_of_trust
		.as_ref()
		.and_then(|rot| rot.verified_boot_hash)
}

/// Ensures the RKP provisioning info approves the build, reports few certificates issued and stamps the vbmeta digest
/// the leaf reports as `verified_boot_hash`.
fn ensure_provisioning_info_accepted(
	info: &[u8],
	verified_boot_hash: Option<&[u8]>,
) -> Result<(), ValidationError> {
	let info = ProvisioningInfo::parse(info).ok_or(ValidationError::ParseProvisioningInfo)?;
	if info.approval_status != Some(b"APPROVED".as_slice()) {
		return Err(ValidationError::BuildNotApproved);
	}
	if info.certificates_issued.is_none_or(|issued| issued >= MAX_CERTIFICATES_ISSUED) {
		return Err(ValidationError::TooManyCertificatesIssued);
	}
	let vbmeta_digest = info
		.vbmeta_digest
		.filter(|digest| digest.len() % 2 == 0 && digest.iter().all(u8::is_ascii_hexdigit))
		.and_then(|digest| core::str::from_utf8(digest).ok())
		.and_then(|digest| impl_serde::serialize::from_hex(digest).ok());
	if vbmeta_digest.is_none() || vbmeta_digest.as_deref() != verified_boot_hash {
		return Err(ValidationError::VerifiedBootHashMismatch);
	}
	Ok(())
}

/// The fields of the RKP provisioning info CBOR map that the validation reads.
#[derive(Debug, PartialEq, Eq)]
struct ProvisioningInfo<'a> {
	/// Field 1: bucketed number of certificates issued to the device in the last 30 days.
	certificates_issued: Option<u64>,
	/// Field 5.1: approval status of the device's build.
	approval_status: Option<&'a [u8]>,
	/// Field 5.2: hex text of the device's vbmeta digest, stamped for approved builds.
	vbmeta_digest: Option<&'a [u8]>,
}

const CBOR_UNSIGNED: u8 = 0;
const CBOR_TEXT: u8 = 3;
const CBOR_MAP: u8 = 5;
const CBOR_MAX_DEPTH: u8 = 8;

impl<'a> ProvisioningInfo<'a> {
	fn parse(data: &'a [u8]) -> Option<Self> {
		if cbor_skip(data, 0, 0)? != data.len() || cbor_head(data, 0)?.0 != CBOR_MAP {
			return None;
		}
		let unsigned = |at| match cbor_head(data, at)? {
			(CBOR_UNSIGNED, value, _) => Some(value),
			_ => None,
		};
		let text = |at| match cbor_head(data, at)? {
			(CBOR_TEXT, len, start) => {
				data.get(start..start.checked_add(usize::try_from(len).ok()?)?)
			},
			_ => None,
		};
		let build_map = cbor_map_get(data, 0, 5);
		let build = |key| build_map.and_then(|field| cbor_map_get(data, field, key));
		Some(Self {
			certificates_issued: cbor_map_get(data, 0, 1).and_then(unsigned),
			approval_status: build(1).and_then(text),
			vbmeta_digest: build(2).and_then(text),
		})
	}
}

/// Index of the value under the unsigned `key` of the well-formed CBOR map at `map`, if it holds one.
fn cbor_map_get(data: &[u8], map: usize, key: u64) -> Option<usize> {
	let (major, entries, mut i) = cbor_head(data, map)?;
	if major != CBOR_MAP {
		return None;
	}
	for _ in 0..entries {
		let (key_major, entry_key, value) = cbor_head(data, i)?;
		if (key_major, entry_key) == (CBOR_UNSIGNED, key) {
			return Some(value);
		}
		i = cbor_skip(data, value, 0)?;
	}
	None
}

/// Reads the CBOR item head at `i`: (major type, argument, index after the head).
fn cbor_head(data: &[u8], i: usize) -> Option<(u8, u64, usize)> {
	let initial = *data.get(i)?;
	let (major, info) = (initial >> 5, initial & 0x1f);
	let size = match info {
		0..=23 => return Some((major, info.into(), i + 1)),
		24 => 1,
		25 => 2,
		26 => 4,
		27 => 8,
		_ => return None,
	};
	let bytes = data.get(i + 1..i + 1 + size)?;
	let argument = bytes.iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b));
	Some((major, argument, i + 1 + size))
}

/// Returns the index after the CBOR item at `i`; indefinite lengths are rejected.
fn cbor_skip(data: &[u8], i: usize, depth: u8) -> Option<usize> {
	if depth > CBOR_MAX_DEPTH {
		return None;
	}
	let (major, argument, mut next) = cbor_head(data, i)?;
	// major types: 0/1 integers, 2/3 byte/text strings, 4/5 arrays/maps, 6 tags, 7 simple values and floats
	match major {
		0 | 1 | 7 => Some(next),
		2 | 3 => {
			let end = next.checked_add(usize::try_from(argument).ok()?)?;
			(end <= data.len()).then_some(end)
		},
		4 | 5 => {
			let items = if major == CBOR_MAP { argument.checked_mul(2)? } else { argument };
			for _ in 0..items {
				next = cbor_skip(data, next, depth + 1)?;
			}
			Some(next)
		},
		6 => cbor_skip(data, next, depth + 1),
		_ => None,
	}
}

const GOOGLE_ROOT_PUB_KEY: &[u8] = include_bytes!("./__root_key__/google-public.key");
const GOOGLE_P384_ROOT_PUB_KEY: &[u8] = include_bytes!("./__root_key__/google-p384-public.key");
const APPLE_ROOT_PUB_KEY: &[u8] = include_bytes!("./__root_key__/apple-public.key");

#[cfg(test)]
fn validate_chain(chain: &CertificateChainInput) -> Result<(), ValidationError> {
	verified_leaf(chain).map(|_| ())
}

/// The leaf of a chain that passes [verify_chain].
#[cfg(test)]
fn verified_leaf(chain: &CertificateChainInput) -> Result<TBSCertificate<'_>, ValidationError> {
	let mut certs = parse_certificate_chain(chain)?;
	verify_chain(&certs)?;
	Ok(certs.pop().ok_or(ValidationError::ChainTooShort)?.cert.tbs_certificate)
}

/// Applies [ensure_chain_policy] alone, with the facts [verify_chain] would report but without its signature checks.
#[cfg(test)]
fn chain_policy(chain: &CertificateChainInput, now: u64) -> Result<bool, ValidationError> {
	let certs = parse_certificate_chain(chain)?;
	let root_included = match certs.first() {
		Some(first) => trusted_roots()?
			.contains(&PublicKey::parse(&first.cert.tbs_certificate.subject_public_key_info)?),
		None => false,
	};
	let leaf_issuer_ca =
		certs.len() < 2 || ensure_ca_certificate(&certs[certs.len() - 2].cert, 0).is_ok();
	let leaf = certs.last().ok_or(ValidationError::ChainTooShort)?;
	let attestation = extract_attestation(&leaf.cert.tbs_certificate.extensions)?;
	ensure_chain_policy(&certs, root_included, leaf_issuer_ca, &attestation, now)
}

#[cfg(test)]
mod tests {
	use core::convert::TryInto;

	use crate::{
		asn::ParsedAttestation,
		attestation::{error::ValidationError, extract_attestation},
		parse_cert, BoundedAttestationContent, BoundedKeyDescription, VerifiedBootState,
	};

	use super::{
		chain_policy, validate_certificate_chain, validate_chain, verified_leaf,
		CertificateChainInput, CertificateInput, P384_COORDINATE_SIZE, P384_UNTAGGED_POINT_SIZE,
	};

	pub fn decode_certificate_chain(chain: &[&str]) -> CertificateChainInput {
		let decoded = chain
			.iter()
			.map(|cert_data| {
				CertificateInput::truncate_from(
					base64::decode(cert_data).expect("error decoding test input"),
				)
			})
			.collect::<Vec<CertificateInput>>();
		CertificateChainInput::truncate_from(decoded)
	}

	/// Regression test for the P-384 public-key parser: a `SubjectPublicKeyInfo` carrying the
	/// secp384r1 OID but an empty public key must be rejected with an error instead of triggering an
	/// out-of-bounds slice panic (`as_bytes()[1..]`) inside [`super::PublicKey::parse`].
	#[test]
	fn parse_p384_public_key_rejects_empty_key() {
		// DER of SEQUENCE {
		//   AlgorithmIdentifier { ecPublicKey (1.2.840.10045.2.1), secp384r1 (1.3.132.0.34) },
		//   BIT STRING (empty)
		// }
		let der = hex_literal::hex!("3015301006072a8648ce3d020106052b81040022030100");
		let spki = asn1::parse_single::<super::SubjectPublicKeyInfo>(&der)
			.expect("crafted SubjectPublicKeyInfo should parse");
		assert!(matches!(super::PublicKey::parse(&spki), Err(ValidationError::ParseP384PublicKey)));
	}

	/// Builds the DER of a `SubjectPublicKeyInfo` carrying the secp384r1 OID with the given raw
	/// bytes as the public key BIT STRING content.
	fn p384_spki_der(public_key: &[u8]) -> Vec<u8> {
		// AlgorithmIdentifier { ecPublicKey (1.2.840.10045.2.1), secp384r1 (1.3.132.0.34) }
		let algorithm = hex_literal::hex!("301006072a8648ce3d020106052b81040022");
		let mut bit_string = vec![0x03, (public_key.len() + 1) as u8, 0x00];
		bit_string.extend_from_slice(public_key);
		let mut der = vec![0x30, (algorithm.len() + bit_string.len()) as u8];
		der.extend_from_slice(&algorithm);
		der.extend_from_slice(&bit_string);
		der
	}

	/// Regression test for the P-384 public-key parser: a compressed point encoding (0x02/0x03
	/// prefix) must be rejected instead of being silently reinterpreted as uncompressed.
	#[test]
	fn parse_p384_public_key_rejects_compressed_key() {
		// compressed point: 0x03 prefix followed by a single x coordinate
		let mut compressed = vec![0x03u8];
		compressed.extend_from_slice(&[0u8; P384_COORDINATE_SIZE]);
		let der = p384_spki_der(&compressed);
		let spki = asn1::parse_single::<super::SubjectPublicKeyInfo>(&der)
			.expect("crafted SubjectPublicKeyInfo should parse");
		assert!(matches!(super::PublicKey::parse(&spki), Err(ValidationError::ParseP384PublicKey)));
	}

	/// Regression test for the P-384 public-key parser: an uncompressed point whose coordinates
	/// are not exactly 96 bytes must be rejected.
	#[test]
	fn parse_p384_public_key_rejects_truncated_key() {
		// uncompressed prefix but one byte short of a full pair of coordinates
		let mut truncated = vec![0x04u8];
		truncated.extend_from_slice(&[0u8; P384_UNTAGGED_POINT_SIZE - 1]);
		let der = p384_spki_der(&truncated);
		let spki = asn1::parse_single::<super::SubjectPublicKeyInfo>(&der)
			.expect("crafted SubjectPublicKeyInfo should parse");
		assert!(matches!(super::PublicKey::parse(&spki), Err(ValidationError::ParseP384PublicKey)));
	}

	/// Regression test for the P-384 public-key parser: a point encoding whose leading byte is
	/// neither the uncompressed (0x04) nor a hybrid (0x06/0x07) tag must be rejected. Before the
	/// leading byte was checked, any prefix followed by a full coordinate pair parsed as an
	/// uncompressed point,
	/// so the same key had 256 accepted encodings.
	#[test]
	fn parse_p384_public_key_rejects_bogus_prefix() {
		// 0x03 is the compressed tag, but with a full pair of coordinates behind it
		let mut bogus = vec![0x03u8];
		bogus.extend_from_slice(&[0u8; P384_UNTAGGED_POINT_SIZE]);
		let der = p384_spki_der(&bogus);
		let spki = asn1::parse_single::<super::SubjectPublicKeyInfo>(&der)
			.expect("crafted SubjectPublicKeyInfo should parse");
		assert!(matches!(super::PublicKey::parse(&spki), Err(ValidationError::ParseP384PublicKey)));
	}

	/// x coordinate of the secp384r1 base point, used as a known on-curve test vector.
	const P384_GENERATOR_X: [u8; P384_COORDINATE_SIZE] = hex_literal::hex!(
		"aa87ca22be8b05378eb1c71ef320ad74"
		"6e1d3b628ba79b9859f741e082542a38"
		"5502f25dbf55296c3a545e3872760ab7"
	);
	/// y coordinate of the secp384r1 base point. It is odd, so it pairs with the 0x07 hybrid tag.
	const P384_GENERATOR_Y: [u8; P384_COORDINATE_SIZE] = hex_literal::hex!(
		"3617de4a96262c6f5d9e98bf9292dc29"
		"f8f41dbd289a147ce9da3113b5f0b8c0"
		"0a60b1ce1d7e819d7a431d7c90ea0e5f"
	);
	/// `p - y` of the base point, i.e. the y coordinate of its negation, which shares the same x
	/// and is therefore also on the curve. It is even, so it pairs with the 0x06 hybrid tag.
	const P384_NEGATED_GENERATOR_Y: [u8; P384_COORDINATE_SIZE] = hex_literal::hex!(
		"c9e821b569d9d390a26167406d6d23d6"
		"070be242d765eb831625ceec4a0f473e"
		"f59f4e30e2817e6285bce2846f15f1a0"
	);

	/// Builds a tagged P-384 point encoding from a tag and a coordinate pair.
	fn p384_point(tag: u8, x: &[u8], y: &[u8]) -> Vec<u8> {
		let mut encoded = vec![tag];
		encoded.extend_from_slice(x);
		encoded.extend_from_slice(y);
		encoded
	}

	/// Parses a raw P-384 point encoding through [`super::PublicKey::parse`].
	fn parse_p384_point(encoded: &[u8]) -> Result<super::PublicKey, ValidationError> {
		let der = p384_spki_der(encoded);
		let spki = asn1::parse_single::<super::SubjectPublicKeyInfo>(&der)
			.expect("crafted SubjectPublicKeyInfo should parse");
		super::PublicKey::parse(&spki)
	}

	/// A well-formed hybrid point (0x06/0x07 prefix, a full coordinate pair) that lies on the
	/// curve is accepted as long as the low bit of the tag matches the parity of y. The base
	/// point has an odd y and its negation an even one, so both tags are exercised on real
	/// points.
	#[test]
	fn parse_p384_public_key_accepts_hybrid_key() {
		for (tag, y) in [(0x07u8, P384_GENERATOR_Y), (0x06u8, P384_NEGATED_GENERATOR_Y)] {
			assert!(matches!(
				parse_p384_point(&p384_point(tag, &P384_GENERATOR_X, &y)),
				Ok(super::PublicKey::ECDSA(super::ECDSACurve::CurveP384(_)))
			));
		}
	}

	/// A hybrid point whose tag contradicts the parity of y is rejected, so a point keeps a single
	/// accepted hybrid encoding.
	#[test]
	fn parse_p384_public_key_rejects_hybrid_key_with_wrong_parity() {
		for (tag, y) in [(0x06u8, P384_GENERATOR_Y), (0x07u8, P384_NEGATED_GENERATOR_Y)] {
			assert!(matches!(
				parse_p384_point(&p384_point(tag, &P384_GENERATOR_X, &y)),
				Err(ValidationError::ParseP384PublicKey)
			));
		}
	}

	/// A well-formed uncompressed point (0x04 prefix, a full coordinate pair) that lies on the
	/// curve is accepted.
	#[test]
	fn parse_p384_public_key_accepts_uncompressed_key() {
		assert!(matches!(
			parse_p384_point(&p384_point(0x04, &P384_GENERATOR_X, &P384_GENERATOR_Y)),
			Ok(super::PublicKey::ECDSA(super::ECDSACurve::CurveP384(_)))
		));
	}

	/// Regression test for the P-384 public-key parser: coordinates that do not satisfy
	/// `y^2 = x^3 - 3x + b` must be rejected. The parser used to build the `AffinePoint` straight
	/// from the raw coordinates, skipping the curve-equation check that the vendored crate only
	/// performs in `FromEncodedPoint`, and handed the off-curve point to certificate signature
	/// verification over a fully caller-supplied chain.
	#[test]
	fn parse_p384_public_key_rejects_off_curve_key() {
		// (0, 0) is not on secp384r1, since `b != 0`
		let zero = [0u8; P384_COORDINATE_SIZE];
		assert!(matches!(
			parse_p384_point(&p384_point(0x04, &zero, &zero)),
			Err(ValidationError::ParseP384PublicKey)
		));

		// a valid x paired with a y that is one bit off the real one
		let mut wrong_y = P384_GENERATOR_Y;
		wrong_y[P384_COORDINATE_SIZE - 1] ^= 0x02;
		assert!(matches!(
			parse_p384_point(&p384_point(0x04, &P384_GENERATOR_X, &wrong_y)),
			Err(ValidationError::ParseP384PublicKey)
		));
	}

	/// UtcTime carries a two-digit year that RFC 5280 pivots at 50 (YY >= 50 means 19YY), and
	/// Keymaster encodes the attestation leaf's "no expiry" date as `691231235959Z` meaning
	/// 2069. Since UtcTime cannot legitimately carry a pre-1970 attestation date, the century is
	/// corrected instead of rejected. It must never clamp to 0, which would make an expired
	/// certificate look valid.
	#[test]
	fn utc_time_before_unix_epoch_gets_century_corrected() {
		// UtcTime "500101000000Z" = 1950-01-01T00:00:00Z, corrected to 2050-01-01T00:00:00Z
		let der = hex_literal::hex!("170d3530303130313030303030305a");
		let time = asn1::parse_single::<super::Time>(&der).expect("UtcTime should parse");
		assert_eq!(time.timestamp_millis(), Ok(2_524_608_000_000));

		// UtcTime "691231235959Z" = 1969-12-31T23:59:59Z, corrected to 2069-12-31T23:59:59Z
		let der = hex_literal::hex!("170d3639313233313233353935395a");
		let time = asn1::parse_single::<super::Time>(&der).expect("UtcTime should parse");
		assert_eq!(time.timestamp_millis(), Ok(3_155_759_999_000));
	}

	/// GeneralizedTime spells the year out in full, so a pre-epoch date there is unambiguous and
	/// must still be rejected rather than shifted.
	#[test]
	fn generalized_time_before_unix_epoch_is_rejected() {
		// GeneralizedTime "19500101000000Z" = 1950-01-01T00:00:00Z
		let der = hex_literal::hex!("180f31393530303130313030303030305a");
		let time = asn1::parse_single::<super::Time>(&der).expect("GeneralizedTime should parse");
		assert_eq!(time.timestamp_millis(), Err(ValidationError::InvalidCertificateDate));
	}

	#[test]
	fn time_after_unix_epoch_is_accepted() {
		// UtcTime "240101000000Z" = 2024-01-01T00:00:00Z
		let der = hex_literal::hex!("170d3234303130313030303030305a");
		let time = asn1::parse_single::<super::Time>(&der).expect("UtcTime should parse");
		assert_eq!(time.timestamp_millis(), Ok(1_704_067_200_000));
	}

	#[test]
	fn leap_day_is_accepted() {
		// UtcTime "240229000000Z" = 2024-02-29T00:00:00Z
		let der = hex_literal::hex!("170d3234303232393030303030305a");
		let time = asn1::parse_single::<super::Time>(&der).expect("UtcTime should parse");
		assert_eq!(time.timestamp_millis(), Ok(1_709_164_800_000));
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
		fn from(_: ValidationError) -> Self {}
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
		let cert = verified_leaf(&decoded_chain)?;
		let parsed_attestation = extract_attestation(&cert.extensions)?;
		if let ParsedAttestation::KeyDescription(key_description) = parsed_attestation {
			assert_eq!(key_description.attestation_version, 100);
			let _: BoundedKeyDescription = key_description.try_into()?;
		} else {
			return Err(());
		}
		Ok(())
	}

	/// A leaf certificate (the attestation key: `cA = FALSE` / no BasicConstraints,
	/// no `keyCertSign`) must be rejected when it appears in a signing position,
	/// otherwise it could be used to forge a sub-certificate that the chain
	/// validation would accept. Regression test for the finding "attestation
	/// certificate-chain verification skips CA/basicConstraints".
	#[test]
	fn test_reject_leaf_cert_used_as_ca() {
		// Valid Samsung chain with the leaf key cert additionally placed in a signing
		// position. The leaf's signature still verifies (it is signed by
		// SAMSUNG_INTERMEDIATE_1_CERT), so the chain can only be rejected by the CA
		// constraint checks, not by signature validation.
		let chain = vec![
			SAMSUNG_ROOT_CERT,
			SAMSUNG_INTERMEDIATE_2_CERT,
			SAMSUNG_INTERMEDIATE_1_CERT,
			SAMSUNG_KEY_CERT,
			SAMSUNG_KEY_CERT,
		];
		let decoded_chain = decode_certificate_chain(&chain);
		assert_eq!(validate_chain(&decoded_chain), Err(ValidationError::BasicConstraintsMissing),);
	}

	/// Legacy chain (2016 Google root) whose batch attestation key cert signing the leaf is
	/// `cA = FALSE` without `keyCertSign`.
	const LEGACY_BATCH_KEY_CHAIN: [&str; 4] = [
		r"MIIFYDCCA0igAwIBAgIJAOj6GWMU0voYMA0GCSqGSIb3DQEBCwUAMBsxGTAXBgNVBAUTEGY5MjAwOWU4NTNiNmIwNDUwHhcNMTYwNTI2MTYyODUyWhcNMjYwNTI0MTYyODUyWjAbMRkwFwYDVQQFExBmOTIwMDllODUzYjZiMDQ1MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAr7bHgiuxpwHsK7Qui8xUFmOr75gvMsd/dTEDDJdSSxtf6An7xyqpRR90PL2abxM1dEqlXnf2tqw1Ne4Xwl5jlRfdnJLmN0pTy/4lj4/7tv0Sk3iiKkypnEUtR6WfMgH0QZfKHM1+di+y9TFRtv6y//0rb+T+W8a9nsNL/ggjnar86461qO0rOs2cXjp3kOG1FEJ5MVmFmBGtnrKpa73XpXyTqRxB/M0n1n/W9nGqC4FSYa04T6N5RIZGBN2z2MT5IKGbFlbC8UrW0DxW7AYImQQcHtGl/m00QLVWutHQoVJYnFPlXTcHYvASLu+RhhsbDmxMgJJ0mcDpvsC4PjvB+TxywElgS70vE0XmLD+OJtvsBslHZvPBKCOdT0MS+tgSOIfga+z1Z1g7+DVagf7quvmag8jfPioyKvxnK/EgsTUVi2ghzq8wm27ud/mIM7AY2qEORR8Go3TVB4HzWQgpZrt3i5MIlCaY504LzSRiigHCzAPlHws+W0rB5N+er5/2pJKnfBSDiCiFAVtCLOZ7gLiMm0jhO2B6tUXHI/+MRPjy02i59lINMRRev56GKtcd9qO/0kUJWdZTdA2XoS82ixPvZtXQpUpuL12ab+9EaDK8Z4RHJYYfCT3Q5vNAXaiWQ+8PTWm2QgBR/bkwSWc+NpUFgNPN9PvQi8WEg5UmAGMCAwEAAaOBpjCBozAdBgNVHQ4EFgQUNmHhAHyIBQlRi0RsR/8aTMnqTxIwHwYDVR0jBBgwFoAUNmHhAHyIBQlRi0RsR/8aTMnqTxIwDwYDVR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMCAYYwQAYDVR0fBDkwNzA1oDOgMYYvaHR0cHM6Ly9hbmRyb2lkLmdvb2dsZWFwaXMuY29tL2F0dGVzdGF0aW9uL2NybC8wDQYJKoZIhvcNAQELBQADggIBACDIw41L3KlXG0aMiS//cqrG+EShHUGo8HNsw30W1kJtjn6UBwRM6jnmiwfBPb8VA91chb2vssAtX2zbTvqBJ9+LBPGCdw/E53Rbf86qhxKaiAHOjpvAy5Y3m00mqC0w/Zwvju1twb4vhLaJ5NkUJYsUS7rmJKHHBnETLi8GFqiEsqTWpG/6ibYCv7rYDBJDcR9W62BW9jfIoBQcxUCUJouMPH25lLNcDc1ssqvC2v7iUgI9LeoM1sNovqPmQUiG9rHli1vXxzCyaMTjwftkJLkf6724DFhuKug2jITV0QkXvaJWF4nUaHOTNA4uJU9WDvZLI1j83A+/xnAJUucIv/zGJ1AMH2boHqF8CY16LpsYgBt6tKxxWH00XcyDCdW2KlBCeqbQPcsFmWyWugxdcekhYsAWyoSf818NUsZdBWBaR/OukXrNLfkQ79IyZohZbvabO/X+MVT3rriAoKc8oE2Uws6DF+60PV7/WIPjNvXySdqspImSN78mflxDqwLqRBYkA3I75qppLGG9rp7UCdRjxMl8ZDBld+7yvHVgt1cVzJx9xnyGCC23UaicMDSXYrB4I4WHXPGjxhZuCuPBLTdOLU8YRvMYdEvYebWHMpvwGCF6bAx3JBpIeOQ1wDB5y0USicV3YgYGmi+NZfhA4URSh77Yd6uuJOJENRaNVTzk",
		r"MIIDwzCCAaugAwIBAgIKA4gmZ2BliZaFdzANBgkqhkiG9w0BAQsFADAbMRkwFwYDVQQFExBmOTIwMDllODUzYjZiMDQ1MB4XDTE2MDUyNjE3MjIxOVoXDTI2MDUyNDE3MjIxOVowGzEZMBcGA1UEBRMQYmU0MDY0NjZiZWEzNzgyYjB2MBAGByqGSM49AgEGBSuBBAAiA2IABNG7VnbdYr9743R76MZBPgjpEy54v1pz7FyAgnATGLfiCjHvXtxmT7uvCB0bUvGPTfU2JMoKGXjxBBVA2RVDN01jZ65EpXntTvctKV9SlwFgubp0w4tB0IJNGo5SxOF1YaOBtjCBszAdBgNVHQ4EFgQUprPevzFgmWV9vksDm3PQsGV3SgEwHwYDVR0jBBgwFoAUNmHhAHyIBQlRi0RsR/8aTMnqTxIwDwYDVR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMCAYYwUAYDVR0fBEkwRzBFoEOgQYY/aHR0cHM6Ly9hbmRyb2lkLmdvb2dsZWFwaXMuY29tL2F0dGVzdGF0aW9uL2NybC9FOEZBMTk2MzE0RDJGQTE4MA0GCSqGSIb3DQEBCwUAA4ICAQCmQbMLG+NeRm6A/rtF1pgD8q8Sd+13RCC24gMBhpdcgSJfPRF4f7Y61d2NL8OlGhVCJoqXvxVXx3lJNYJAwLeZIKk9iwZaJ7BI6zpxJUFaaGzf6793Z1jNLeqAMBbluxbSik+ZHdY1m/2kjLKqOIrYniCubahKl/OwsQeTJibATNaSe1IbSxZGBgzgRYtpQGIUvPk4wJ9Zs1Z/drsAZ70LciS3T+68WFjWiBzoXZ1JBG9BpR0zQzkxN2jkSAtBmGLagR1WGbxwwehThcBi7cGCJVGGFv0TbvZOtmoaa0E+zPcDz0HTxk+YalABrLeUrRC9DtEl+lwM8TNzruG7/+r89Wga40/WxcTrdAFCExsKx6Q3RC5wMVvesUSlsa9nndLUzZbRdiyS+x477gqDt6JwQ2ZLYxpt0D94czJd9KY4YY246i/ge0ggAfrozoOquXWnfHb0j73SS6ZFU8uX6o8+M9Aac/ZMcLVFeXUCF67Sxjce0cFRKaFs63Dhph8MKjIvtsue+LxhCJ8agrULkqvNeT+6TxYicH8Hj7K8BJmlVHsM35o90OHQtn5h755+Fjtrgcvnk0o1C/cfc4WzejGEgDuY9M6Z6kxdNKdomAV8YupF2PLwjdessamdr66tyt08Tea1zsxJ6e6Z/jkRFtAKbAUsfeqAD+aBWNgu5/XzFg==",
		r"MIICLDCCAbKgAwIBAgIKAnIUWGVwUJBgaDAKBggqhkjOPQQDAjAbMRkwFwYDVQQFExBiZTQwNjQ2NmJlYTM3ODJiMB4XDTE2MDUyNjE3Mzc1M1oXDTI2MDUyNDE3Mzc1M1owGzEZMBcGA1UEBRMQYjczYWNmMWQyOTk4ZGIyZDBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABIS6/x81v7FO7EGxFiIxf/J11zeXCWhv10toXiAYm9zr+h/qbQ8oipabJJOYPCFFrxMKrO9M/CiUEnbLckwf86+jgd0wgdowHQYDVR0OBBYEFJ5oNybcwGYLSp8dmSSxm7QUO7QiMB8GA1UdIwQYMBaAFKaz3r8xYJllfb5LA5tz0LBld0oBMAwGA1UdEwEB/wQCMAAwDgYDVR0PAQH/BAQDAgeAMCQGA1UdHgQdMBugGTAXghVpbnZhbGlkO2VtYWlsOmludmFsaWQwVAYDVR0fBE0wSzBJoEegRYZDaHR0cHM6Ly9hbmRyb2lkLmdvb2dsZWFwaXMuY29tL2F0dGVzdGF0aW9uL2NybC8wMjcyMTQ1ODY1NzA1MDkwNjA2ODAKBggqhkjOPQQDAgNoADBlAjBB6GkFvjSLoMxkPr11ypz72LtCNEwdWbMEwG3buL2KAfOqGrHjNEAbPhjsKTaDKMMCMQCMjM4sETuaM5BcMkzUaXGsUwR4D7ZxCSApCWensq9flVJ2no5Ztd5gAimZBnmrz9Y=",
		r"MIICmDCCAj6gAwIBAgIBATAKBggqhkjOPQQDAjAbMRkwFwYDVQQFExBiNzNhY2YxZDI5OThkYjJkMCAXDTcwMDEwMTAwMDAwMFoYDzIxMDYwMjA3MDYyODE1WjAfMR0wGwYDVQQDDBRBbmRyb2lkIEtleXN0b3JlIEtleTBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABKdnHAjTKTKdmFXuvwLJL9oguKNEfcFoA+F1DIL4cXAVAgrnHognQ/ZIhEUyvB/5COmc9U2eGe2FN7ouzGHi0fqjggFrMIIBZzAOBgNVHQ8BAf8EBAMCB4AwggFTBgorBgEEAdZ5AgERBIIBQzCCAT8CAQMKAQECAQQKAQEEIKVnboqsOc7mtpKSdszrSn7ccL/fRlFfcTF/KUOaATbaBAAwZr+FPQgCBgGhHe7CSL+FRVYEVDBSMSwwKgQkY29tLmFjdXJhc3QuYXR0ZXN0ZWQuZXhlY3V0b3IuY2FuYXJ5AgIAjDEiBCDscMKk4HKg9YZVKmg1eyNpfJ1F8eEleoxNKaJaxJgkMzCBpKEIMQYCAQICAQOiAwIBA6MEAgIBAKUFMQMCAQCqAwIBAb+DdwIFAL+FPgMCAQC/hUBMMEoEIAlBlF/vs/y30TeRTnxfTkEMUccL2P1fG/ISCoFv2O1rAQH/CgEABCCRIdOl33rZqDw36czFIGrK/GOec7BocWybuorv5MQytL+FQQUCAwHUwL+FQgUCAwMV47+FTgYCBAE0jK2/hU8GAgQBNIytMAoGCCqGSM49BAMCA0gAMEUCIQCVqGU51RrdS+FGTQFoFrTLY1XL5Nz4/gfFNw643TrP5AIgeSpCgcEtPnDxTMEPs+fktsCPICAJfvFolDOwxPZVUps=",
	];

	#[test]
	fn test_validate_legacy_batch_key_chain() -> Result<(), Error> {
		let decoded_chain = decode_certificate_chain(&LEGACY_BATCH_KEY_CHAIN);
		// onboarding time of the chain; both intermediates expired 2026-05-24
		let chain = validate_certificate_chain(&decoded_chain, 1_791_503_598_000)?;
		assert!(!chain.is_rkp);
		let ParsedAttestation::KeyDescription(_) = chain.attestation else {
			return Err(());
		};
		Ok(())
	}

	/// A non-CA issuer of the leaf is only tolerated in a keybox chain.
	#[test]
	fn test_reject_legacy_batch_key_outside_keybox_chain() {
		let mut chain = LEGACY_BATCH_KEY_CHAIN[..3].to_vec();
		chain.push(APPLE_KEY_CERT);
		let decoded_chain = decode_certificate_chain(&chain);
		// within the validity of the Apple leaf and the legacy intermediates
		assert_eq!(
			chain_policy(&decoded_chain, 1_735_689_600_000),
			Err(ValidationError::NotACertificateAuthority),
		);
	}

	const APPLE_KEY_CERT: &str = r"MIIDKjCCArCgAwIBAgIGAZHq9k9GMAoGCCqGSM49BAMCME8xIzAhBgNVBAMMGkFwcGxlIEFwcCBBdHRlc3RhdGlvbiBDQSAxMRMwEQYDVQQKDApBcHBsZSBJbmMuMRMwEQYDVQQIDApDYWxpZm9ybmlhMB4XDTI0MDkxMjEwMzgxNloXDTI1MDcyNDA0MjAxNlowgZExSTBHBgNVBAMMQDY4NTUzY2RkZmU2N2QyMTIxNzFhZGIyMjQ1OTI5NzFhNmM5MGQ0YzJlNzRhMWU4Y2MyNjU3NzI5YzQwNjhhYjUxGjAYBgNVBAsMEUFBQSBDZXJ0aWZpY2F0aW9uMRMwEQYDVQQKDApBcHBsZSBJbmMuMRMwEQYDVQQIDApDYWxpZm9ybmlhMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE7UseYUwbGOo2uUDv2EyC/mYbE2qANYlGVS0TOMeN4NQ8bdyb0CdxjqZ0p91Le58fLN/AhA1hyZV9HuV81oAKHKOCATMwggEvMAwGA1UdEwEB/wQCMAAwDgYDVR0PAQH/BAQDAgTwMIGABgkqhkiG92NkCAUEczBxpAMCAQq/iTADAgEBv4kxAwIBAL+JMgMCAQG/iTMDAgEBv4k0IQQfR1YyNDUyOTIyUi5jb20uYWN1cmFzdC5leGVjdXRvcqUGBARza3Mgv4k2AwIBBb+JNwMCAQC/iTkDAgEAv4k6AwIBAL+JOwMCAQAwVwYJKoZIhvdjZAgHBEowSL+KeAgEBjE3LjYuMb+IUAcCBQD////+v4p7BwQFMjFHOTO/in0IBAYxNy42LjG/in4DAgEAv4sMDwQNMjEuNy45My4wLjAsMDAzBgkqhkiG92NkCAIEJjAkoSIEIOGFqR92fg26fqKg+3a8RKWltjU3rcAJfq2lX2RtjBPyMAoGCCqGSM49BAMCA2gAMGUCMQCo9hwAAF7Kw+Gb9uLy3bwDdoIYTSQUCsyAvmsUduEuQSCi3OClMOaQxEksDIZRGlMCMBGkWYyKDk2PgspcBKBr8xG/VpGKQ2QAJlUgnrEqbEvtA7LomzGoj0TRbaq4dS4G5A==";

	/// The non-CA batch key is only tolerated directly above the last certificate.
	#[test]
	fn test_reject_legacy_batch_key_above_intermediate() {
		let mut chain = LEGACY_BATCH_KEY_CHAIN.to_vec();
		chain.push(LEGACY_BATCH_KEY_CHAIN[3]);
		let decoded_chain = decode_certificate_chain(&chain);
		assert_eq!(validate_chain(&decoded_chain), Err(ValidationError::NotACertificateAuthority),);
	}

	#[test]
	fn test_validate_samsung_chain_2() -> Result<(), Error> {
		let decoded_chain = [
            hex_literal::hex!("3082051c30820304a003020102020900d50ff25ba3f2d6b3300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3139313132323230333735385a170d3334313131383230333735385a301b31193017060355040513106639323030396538353362366230343530820222300d06092a864886f70d01010105000382020f003082020a0282020100afb6c7822bb1a701ec2bb42e8bcc541663abef982f32c77f7531030c97524b1b5fe809fbc72aa9451f743cbd9a6f1335744aa55e77f6b6ac3535ee17c25e639517dd9c92e6374a53cbfe258f8ffbb6fd129378a22a4ca99c452d47a59f3201f44197ca1ccd7e762fb2f53151b6feb2fffd2b6fe4fe5bc6bd9ec34bfe08239daafceb8eb5a8ed2b3acd9c5e3a7790e1b51442793159859811ad9eb2a96bbdd7a57c93a91c41fccd27d67fd6f671aa0b815261ad384fa37944864604ddb3d8c4f920a19b1656c2f14ad6d03c56ec060899041c1ed1a5fe6d3440b556bad1d0a152589c53e55d370762f0122eef91861b1b0e6c4c80927499c0e9bec0b83e3bc1f93c72c049604bbd2f1345e62c3f8e26dbec06c94766f3c128239d4f4312fad8123887e06becf567583bf8355a81feeabaf99a83c8df3e2a322afc672bf120b135158b6821ceaf309b6eee77f98833b018daa10e451f06a374d50781f359082966bb778b9308942698e74e0bcd24628a01c2cc03e51f0b3e5b4ac1e4df9eaf9ff6a492a77c1483882885015b422ce67b80b88c9b48e13b607ab545c723ff8c44f8f2d368b9f6520d31145ebf9e862ad71df6a3bfd2450959d653740d97a12f368b13ef66d5d0a54a6e2f5d9a6fef446832bc67844725861f093dd0e6f3405da89643ef0f4d69b6420051fdb93049673e36950580d3cdf4fbd08bc58483952600630203010001a3633061301d0603551d0e041604143661e1007c880509518b446c47ff1a4cc9ea4f12301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff040403020204300d06092a864886f70d01010b050003820201004e31a05cf28ba65dbdafa1ced70969ee5ca84104added8a306cf7f6dee50375d745ed992cb0242cce72dc9eed51191fe5ad52bad7dd3b25c099e13a491a3cdd487a5acce8766324c4ae46338246ae7b78a418acbb98a05c4c9d696eeaab609d0ba0ce1a31be98490df3f4c0ea9ddc9e82ffb0fcb3e9ebdd8cb952789f2b1411fac56c886426eb7296042735da50e11ac715f1818cf9fdc4e254a3763351b6a2440150861263a6e310be1a50de5c7e8ee880fdd4be5884a37128d18830bb3476bf4291e82d5c66a6494939e08480bfbc00f7d8a74d43e73737ebe5d8e4ec515302d4689692780dc7538ed7e9175be6139e74d43ad388b3050ffd5a9de5262000898c01f63c53dfe22209108fa4f65ba16c49ccbde0837d7c5844d54b7398ba0122e505b155c9313cfe26e72d87e22aa1616e6bdbf547ddff93df29e35a63b455fe1fc0ec95581f3f4f7bbe3bb828396a37ae3157582bc3764b9780a239efc0f75a1e2e6d941ceabac27ddeb01e2bd8421029bea34d51aee6c60271d5a95ebd00515a9c0013dd80bf87eea260b81c34f688e6eb1348af0d8ea1cac32acb9d93fa24aff030a84c8f2b0f569cc95080b20ac35ace0c6d8dbd4f6847719519d32450166eb4bf15b859044501adeaf436382c34b15e3b54c92e61b69c2bfc7264589172b3c93dbe35ce06d08fd5c01322ca0877b1d12743af1fad5940ea1bc02dd891c").to_vec().try_into().unwrap(),
            hex_literal::hex!("3082039930820181a00302010202105f641897b6861d5994a20e80ba2a7b08300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3231303931353232353733305a170d3331303931333232353733305a303f31123010060355040c0c095374726f6e67426f78312930270603550405132031383464643461376438613731333165393861616534656137616635313064613076301006072a8648ce3d020106052b8104002203620004e242634c3b59ae131662e099fec272ec720d7642cdcc042f5f605cffddbe73b36822d2181fd27684afc25ba59983efa91a60f901a0097545163a7654c662f62e9059d97cc14e592d5efc69ec25cac3ccbda903fd511b1c4b13825d2f19e92a59a3633061301d0603551d0e0416041448e9cf69260a89c4c584499c6dbad846e5a266b3301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff040403020204300d06092a864886f70d01010b050003820201004931dd12fbf627e89b0cce272e83b0071ca0538b42d8001e34f8ddc710fba237c7040361520769fda10a17501db710dc1894ccbf1fff6f4bcedff0a7708fe6b40fa456b9f528d46dcc04a2167b7c130f649ac1f1756794438b315d15877fe790253f13e4023979bb0688bc44af9f09eeee1a4d9f0339f3508f689ba24026c7e59e5b527740762fa87e76d70b6ab24ad4fb2f2d7dd4ac4d0b69226a9a8a124a3bfe5319f16c530aec2508537dec72ea7bc9d0b75eb4cccc70844912d8089a56b21d6d64a1ae55c3ae6c5f9a3a8bbed5d0c64501071082f1df33997bbc5bdb1461fcf5eeb3ffc39f47d15b24d3bdd3904963b37197b6773cfe314b94a653f691ba5294cf6df110b627100d1579f7b6ef8b3b643d0606e072869de7ce6d5d6fde1ff7ee60c251b0f633cb20203fe8243857b0fbcecfcd92f8dd6db31aa5022c695fa912f4de1782e2c42c32aa8e7bf4f8d2626cc151336871fde59f85ce66a193a7a2d8e3529ef03a21ee4bdbc2880772831cf0fa53a301f11efa1951eaaca5c5a7740d1ab5e206b411b27f23cd93fab10e0ee40c08ef61fb8d18e521eb867fc169e64af8be6e79fb5923c706541bfe44cf76bf7a6774f3b4b9e8b9876eca4a1bd896490856f61f1678d06c176dafc228eeb797052b4b24d320879e1ee5181f763a4061cf77f9b5034a8ce317ea7ce1b8d10061d14d39acf8fdb463dbcd44ac5524").to_vec().try_into().unwrap(),
            hex_literal::hex!("308201fe30820185a00302010202106fd912a55cb7563380ddfa2ac2d1eb8c300a06082a8648ce3d040302303f31123010060355040c0c095374726f6e67426f7831293027060355040513203138346464346137643861373133316539386161653465613761663531306461301e170d3231303931353232353831375a170d3331303931333232353831375a303f31123010060355040c0c095374726f6e67426f78312930270603550405132037386235656561343461313465353437373261363862653665623738313462343059301306072a8648ce3d020106082a8648ce3d03010703420004df1afbe9c0d1fb4d291a6573de2cdf95ca9f43e4bd2f33ded358ca8118dd42ea619a9cdbccd4cc32335f5db51774bb5e9ccf08a2d6d3259572a27fe064fd451da3633061301d0603551d0e04160414f40c71002bcb6425158db7b4b552d09820d9e4c9301f0603551d2304183016801448e9cf69260a89c4c584499c6dbad846e5a266b3300f0603551d130101ff040530030101ff300e0603551d0f0101ff040403020204300a06082a8648ce3d0403020367003064023044f91fe977e5ddba9c69a4f0b0d9b7c550e8e117216d6768b3477e4277fb88e9ed1dcae08c538b5452f30f4aabd473c10230312efaac05f998263283e0b718025eaa224ee8e386b6f143b9781c789f3a38abc8480731325893d6efc25b67c473ecf8").to_vec().try_into().unwrap(),
            hex_literal::hex!("308202ba30820260a003020102020101300a06082a8648ce3d040302303f31123010060355040c0c095374726f6e67426f7831293027060355040513203738623565656134346131346535343737326136386265366562373831346234301e170d3730303130313030303030305a170d3439313233313233353935395a301f311d301b06035504031314416e64726f6964204b657973746f7265204b65793059301306072a8648ce3d020106082a8648ce3d03010703420004e203b4ed148733aca6322978ffb9d72dc940919d489d87ae5242cf6eb39b6ae8ab9edf67310b5b88e8ab5e82f7ec6cb7778ad00c480e4846aa6dbbde55303b43a382016b30820167300c0603551d0f0405030307880030820155060a2b06010401d67902011104820145308201410201640a01020201640a0102042052331675bfbad8106ff561287fcb0397307dc4ed8cccc3f7e2ace4a7fe82f19c04003065bf853d080206018c1a85f13ebf85455504533051312b30290424636f6d2e616375726173742e61747465737465642e6578656375746f722e63616e61727902011031220420ec70c2a4e072a0f586552a68357b23697c9d45f1e1257a8c4d29a25ac49824333081a7a10b3109020106020103020102a203020103a30402020100a5053103020100aa03020101bf8377020500bf853e03020100bf85404c304a0420c276f9fcf895a8838c8d6e6ec441494822e69acfca3bb27715790b2951da33980101ff0a010004209b51df228d989cd600b59e307b0bbc1d92013f969d4cebcc0ca3e1556bff6e7bbf854105020301fbd0bf854205020303163fbf854e0602040134b09dbf854f0602040134b09d300a06082a8648ce3d040302034800304502200229006e3a528c45224739b7773731c99ca811fa3ee57121626bbc9279fad3af022100bf47d355feae07bb6a4528e8872a8775248d4960477ce807cf4ee7ce902d42b9").to_vec().try_into().unwrap(),
        ].to_vec().try_into().unwrap();
		let cert = verified_leaf(&decoded_chain)?;
		let parsed_attestation = extract_attestation(&cert.extensions)?;
		if let ParsedAttestation::KeyDescription(key_description) = parsed_attestation {
			assert_eq!(key_description.attestation_version, 100);
			let _: BoundedKeyDescription = key_description.try_into()?;
		} else {
			return Err(());
		}
		Ok(())
	}

	#[test]
	fn test_solana_seeker_chain() -> Result<(), Error> {
		let decoded_chain = [
            hex_literal::hex!("3082051c30820304a003020102020900d50ff25ba3f2d6b3300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3139313132323230333735385a170d3334313131383230333735385a301b31193017060355040513106639323030396538353362366230343530820222300d06092a864886f70d01010105000382020f003082020a0282020100afb6c7822bb1a701ec2bb42e8bcc541663abef982f32c77f7531030c97524b1b5fe809fbc72aa9451f743cbd9a6f1335744aa55e77f6b6ac3535ee17c25e639517dd9c92e6374a53cbfe258f8ffbb6fd129378a22a4ca99c452d47a59f3201f44197ca1ccd7e762fb2f53151b6feb2fffd2b6fe4fe5bc6bd9ec34bfe08239daafceb8eb5a8ed2b3acd9c5e3a7790e1b51442793159859811ad9eb2a96bbdd7a57c93a91c41fccd27d67fd6f671aa0b815261ad384fa37944864604ddb3d8c4f920a19b1656c2f14ad6d03c56ec060899041c1ed1a5fe6d3440b556bad1d0a152589c53e55d370762f0122eef91861b1b0e6c4c80927499c0e9bec0b83e3bc1f93c72c049604bbd2f1345e62c3f8e26dbec06c94766f3c128239d4f4312fad8123887e06becf567583bf8355a81feeabaf99a83c8df3e2a322afc672bf120b135158b6821ceaf309b6eee77f98833b018daa10e451f06a374d50781f359082966bb778b9308942698e74e0bcd24628a01c2cc03e51f0b3e5b4ac1e4df9eaf9ff6a492a77c1483882885015b422ce67b80b88c9b48e13b607ab545c723ff8c44f8f2d368b9f6520d31145ebf9e862ad71df6a3bfd2450959d653740d97a12f368b13ef66d5d0a54a6e2f5d9a6fef446832bc67844725861f093dd0e6f3405da89643ef0f4d69b6420051fdb93049673e36950580d3cdf4fbd08bc58483952600630203010001a3633061301d0603551d0e041604143661e1007c880509518b446c47ff1a4cc9ea4f12301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff040403020204300d06092a864886f70d01010b050003820201004e31a05cf28ba65dbdafa1ced70969ee5ca84104added8a306cf7f6dee50375d745ed992cb0242cce72dc9eed51191fe5ad52bad7dd3b25c099e13a491a3cdd487a5acce8766324c4ae46338246ae7b78a418acbb98a05c4c9d696eeaab609d0ba0ce1a31be98490df3f4c0ea9ddc9e82ffb0fcb3e9ebdd8cb952789f2b1411fac56c886426eb7296042735da50e11ac715f1818cf9fdc4e254a3763351b6a2440150861263a6e310be1a50de5c7e8ee880fdd4be5884a37128d18830bb3476bf4291e82d5c66a6494939e08480bfbc00f7d8a74d43e73737ebe5d8e4ec515302d4689692780dc7538ed7e9175be6139e74d43ad388b3050ffd5a9de5262000898c01f63c53dfe22209108fa4f65ba16c49ccbde0837d7c5844d54b7398ba0122e505b155c9313cfe26e72d87e22aa1616e6bdbf547ddff93df29e35a63b455fe1fc0ec95581f3f4f7bbe3bb828396a37ae3157582bc3764b9780a239efc0f75a1e2e6d941ceabac27ddeb01e2bd8421029bea34d51aee6c60271d5a95ebd00515a9c0013dd80bf87eea260b81c34f688e6eb1348af0d8ea1cac32acb9d93fa24aff030a84c8f2b0f569cc95080b20ac35ace0c6d8dbd4f6847719519d32450166eb4bf15b859044501adeaf436382c34b15e3b54c92e61b69c2bfc7264589172b3c93dbe35ce06d08fd5c01322ca0877b1d12743af1fad5940ea1bc02dd891c").to_vec().try_into().unwrap(),
            hex_literal::hex!("3082038030820168a003020102020a0388266760658996860f300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3232303132363232353032305a170d3337303132323232353032305a302931133011060355040a130a476f6f676c65204c4c43311230100603550403130944726f6964204341323076301006072a8648ce3d020106052b8104002203620004fedfb8008e39e03f29337d99504a6e692d1ec0b8c53fd1013a7085e0a933da3a9c0c40a9d1f8f2df801a4c28099e919d08b214deefd65c1b37a4410280cb92f515522aa8f9f38c1d6e97318896a16564b31ea3cad4d9f92d961d408a4b27d714a3663064301d0603551d0e04160414399807063a33129ef514063a80410c7180ce1aad301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f1230120603551d130101ff040830060101ff020102300e0603551d0f0101ff040403020106300d06092a864886f70d01010b050003820201003d053b9f20c1641be913a9e1cee190ec108968824c6d9477d370c3970a96c314f189593a4f275e7e4ec007ee153d352a071b6aed423431025fba80d6f27243d1c1b28a9aedf9a2d1c7f58ff45106fbc0b86be0df3472d3e39c930236f37db7f7d97ff3514c4a14be19ccd359f84b258fce6e45411646a7442a43844fd2c4d1cbff99c9cd8687f0e76300ab9f9c3f66705c98bf38e2c5b70f10ee667a51b093b29f0b1f66892a55b81ea637535da02b128bde99610181959b04f282dd6b37240b2813be9dacefb1ad22cc32c0e1dfdc711adfd3909bc07c836707732e2ee642a815e48f23dce8ea3c7e5836f512bf202994f5bbb96bc025be25442ab986c61cbcd20c8b117c6874b138e8447fece290f7ef539802490f307560b81644f22bf9bafd23e0807037f78bca841bd63981bff744514f5c79777fb3c6f9fbc93b3d8858703300d28c8541cbeb84ba166eacbb06b636e62584639c47e84b0716b7b9be9d6cbb5ed1766c5bcb79edee65f3d4dc4141d037accfa471995f70af6c1d33554c8dbdd16cbd212fbcff16a5aed6a5a383c0e84c287f25f2d7c5c8f4075d36cef9e0a3dff3fc8031b55cff6682966d5117bffbbc1d1fc7779098b138592d354acaac5a70ac1c249bca3748688c8a045421001b482312cd60b170fb49e703b0c153f3357bd81604b87c773d711c5a3e4e7e5e758698d5b632865181294286e0cf25").to_vec().try_into().unwrap(),
            hex_literal::hex!("308201d63082015ca003020102021335772a634a0c9d3bc9466901d17cffad747726300a06082a8648ce3d040303302931133011060355040a130a476f6f676c65204c4c43311230100603550403130944726f696420434132301e170d3235303732323036353032305a170d3235303933303036353031395a302931133011060355040a130a476f6f676c65204c4c43311230100603550403130944726f6964204341333059301306072a8648ce3d020106082a8648ce3d03010703420004fabf9ee100e9e3c72017e407feb4da4f1a7c76e10edac856b18b87633071522fc612043b46cd706a39faa3120ee1c706722afa5303639893c9e9bcc18760355fa3633061300e0603551d0f0101ff040403020204300f0603551d130101ff040530030101ff301d0603551d0e04160414b4c77197661ea2bc8f9eaa483f1425effb9b71e3301f0603551d23041830168014399807063a33129ef514063a80410c7180ce1aad300a06082a8648ce3d0403030368003065023016e32066adfe2ff15de40ee7966cbe32953f8f56f5e4fd60204a3a8a7334b82c00552d789963b93e7315143fd1f70ab7023100b0af29dfe98a1bc46fe834858afc840e3d5e321856f3585e61750a3426910f78ea62afd1f731788f28ee243c223d00a9").to_vec().try_into().unwrap(),
            hex_literal::hex!("308201f230820198a00302010202106b1a0d4882cf7b565006bd71a77f1159300a06082a8648ce3d040302302931133011060355040a130a476f6f676c65204c4c43311230100603550403130944726f696420434133301e170d3235303831393130323934305a170d3235303931363132353835385a303f3129302706035504031320366231613064343838326366376235363530303662643731613737663131353931123010060355040a13095374726f6e67426f783059301306072a8648ce3d020106082a8648ce3d0301070342000409a73a592725d66795565da358102607a14b1a423f17a942981d7db5914d98f691860c02226093f5f502963e40422229a5db0247140150160cfd813d1d550915a3818b308188301d0603551d0e04160414311db33cf32f95bafc64804d5d887be37b49f9eb301f0603551d23041830168014b4c77197661ea2bc8f9eaa483f1425effb9b71e3300f0603551d130101ff040530030101ff300e0603551d0f0101ff0404030202043025060a2b06010401d67902011e0417a201080372536f6c616e61204d6f62696c6520496e632e300a06082a8648ce3d0403020348003045022100c6b1f86dedcaf4f8bc9dd669869c2d4ffb3c991b754a2da2a8b876c6aeedde2c02205ee0cef4add14256e4d9470a2d7979a1e8421c9ee531636b2293630d88b14442").to_vec().try_into().unwrap(),
            hex_literal::hex!("308202bf30820265a003020102020101300a06082a8648ce3d040302303f3129302706035504031320366231613064343838326366376235363530303662643731613737663131353931123010060355040a13095374726f6e67426f78301e170d3730303130313030303030305a170d3438303130313030303030305a301f311d301b06035504031314416e64726f6964204b657973746f7265204b65793059301306072a8648ce3d020106082a8648ce3d03010703420004525fd90b61574ff43b0a9f8833674e0063e52f07fdbaa6e91810f92a4680d2c90634e7c527077bd8575d2a7b66a3eb6cf8771fee4f9663f5f41ba0d861dc6787a38201703082016c30820158060a2b06010401d67902011104820148308201440202012c0a01020202012c0a01020420dc1d47687c0142ce02b1a7c3240d689269fc365bd6c49aa7ab3643a6e730809504003069bf853d0802060198cc525cd1bf85455904573055312f302d0428636f6d2e616375726173742e61747465737465642e6578656375746f722e7362732e63616e61727902015331220420e095733f011ae6934a02d65a0945fcf24c16af7598c1c23405dcc4f3cb9ee5bc3081a4a1083106020103020102a203020103a30402020100a5053103020100aa03020101bf8377020500bf853e03020100bf85404c304a0420eb63f1a60094029d2e5e3968dadaff3407fb4280394869cddbb3e64bf17ae37b0101ff0a010004207b8301dc8ef68779083c58875fdbc71aa8ba5bd0309160d85bc6c21a848085efbf85410502030249f0bf854205020303170abf854e0602040134ffedbf854f0602040134d815300e0603551d0f010101040403020780300a06082a8648ce3d0403020348003045022100e6f8fbaacf03b67b6f2f514cac6ba0f408762670e1bc8154748876b420775a5402204d44d0ba2bc4477442bcb63e54877c9cc361b65e58753f0440c43c2fdd71ae19").to_vec().try_into().unwrap(),
        ].to_vec().try_into().unwrap();
		let cert = verified_leaf(&decoded_chain)?;
		_ = extract_attestation(&cert.extensions)?;
		Ok(())
	}

	#[test]
	fn expired_trusted_root_is_ignored() {
		// the 2016 Google root expired 2026-05-24; the intermediates and the leaf are still valid
		let chain = decode_certificate_chain(&[
			PIXEL_ROOT_CERT,
			PIXEL_INTERMEDIATE_2_CERT,
			PIXEL_INTERMEDIATE_1_CERT,
			PIXEL_KEY_CERT,
		]);
		assert_eq!(chain_policy(&chain, 1_800_000_000_000), Ok(false));
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
			let cert = verified_leaf(&decoded_chain).expect("validating chain failed");
			let parsed_attestation = extract_attestation(&cert.extensions)?;
			if let ParsedAttestation::KeyDescription(key_description) = parsed_attestation {
				match key_description.attestation_version {
					4 | 100 | 200 | 300 => {},
					_ => return Err(()),
				}
				let _: BoundedKeyDescription = key_description.try_into()?;
			} else {
				return Err(());
			}
		}
		Ok(())
	}

	/// Samsung chain omitting the root cert. The Samsung root carries the same
	/// RSA public key as GOOGLE_ROOT_PUB_KEY, so SAMSUNG_INTERMEDIATE_2_CERT
	/// (which is signed by that root) should be accepted as the chain head.
	#[test]
	fn test_validate_samsung_chain_without_root() -> Result<(), Error> {
		let chain =
			vec![SAMSUNG_INTERMEDIATE_2_CERT, SAMSUNG_INTERMEDIATE_1_CERT, SAMSUNG_KEY_CERT];
		let decoded_chain = decode_certificate_chain(&chain);
		let cert = verified_leaf(&decoded_chain)?;
		let parsed_attestation = extract_attestation(&cert.extensions)?;
		if let ParsedAttestation::KeyDescription(key_description) = parsed_attestation {
			assert_eq!(key_description.attestation_version, 100);
			let _: BoundedKeyDescription = key_description.try_into()?;
		} else {
			return Err(());
		}
		Ok(())
	}

	/// Pixel chain omitting the root cert. PIXEL_INTERMEDIATE_2_CERT is signed
	/// by the Google RSA root and should pass direct validation against
	/// GOOGLE_ROOT_PUB_KEY.
	#[test]
	fn test_validate_pixel_chain_without_root() -> Result<(), Error> {
		let chain = vec![PIXEL_INTERMEDIATE_2_CERT, PIXEL_INTERMEDIATE_1_CERT, PIXEL_KEY_CERT];
		let decoded_chain = decode_certificate_chain(&chain);
		validate_chain(&decoded_chain)?;
		Ok(())
	}

	/// A chain that starts with an intermediate that is NOT signed by any
	/// trusted root must be rejected. Here we supply the second-level Samsung
	/// intermediate as the head, which was signed by the first-level
	/// intermediate (not by the root), so none of the embedded trusted root
	/// keys can verify it.
	#[test]
	fn test_validate_chain_without_root_invalid_first_intermediate() -> Result<(), ()> {
		// SAMSUNG_INTERMEDIATE_1_CERT is signed by SAMSUNG_INTERMEDIATE_2_CERT,
		// not by the Google/Samsung root, so it must not be accepted as a chain head.
		let chain = vec![SAMSUNG_INTERMEDIATE_1_CERT, SAMSUNG_KEY_CERT];
		let decoded_chain = decode_certificate_chain(&chain);
		match validate_chain(&decoded_chain) {
			Err(ValidationError::InvalidSignature) => Ok(()),
			_ => Err(()),
		}
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
		let res = validate_chain(&decoded_chain);
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
		let res = validate_chain(&decoded_chain);
		match res {
			Err(e) => assert_eq!(e, ValidationError::InvalidSignature),
			_ => return Err(()),
		};
		Ok(())
	}

	#[test]
	fn test_validate_ios_chain() -> Result<(), ()> {
		let chain = [
			hex_literal::hex!("30820243308201c8a003020102021009bac5e1bc401ad9d45395bc381a0854300a06082a8648ce3d04030330523126302406035504030c1d4170706c6520417070204174746573746174696f6e20526f6f7420434131133011060355040a0c0a4170706c6520496e632e3113301106035504080c0a43616c69666f726e6961301e170d3230303331383138333935355a170d3330303331333030303030305a304f3123302106035504030c1a4170706c6520417070204174746573746174696f6e204341203131133011060355040a0c0a4170706c6520496e632e3113301106035504080c0a43616c69666f726e69613076301006072a8648ce3d020106052b8104002203620004ae5b37a0774d79b2358f40e7d1f22626f1c25fef17802deab3826a59874ff8d2ad1525789aa26604191248b63cb967069e98d363bd5e370fbfa08e329e8073a985e7746ea359a2f66f29db32af455e211658d567af9e267eb2614dc21a66ce99a366306430120603551d130101ff040830060101ff020100301f0603551d23041830168014ac91105333bdbe6841ffa70ca9e5faeae5e58aa1301d0603551d0e041604143ee35d1c0419a9c9b431f88474d6e1e15772e39b300e0603551d0f0101ff040403020106300a06082a8648ce3d0403030369003066023100bbbe888d738d0502cfbcfd666d09575035bcd6872c3f8430492629edd1f914e879991c9ae8b5aef8d3a85433f7b60d06023100ab38edd0cc81ed00a452c3ba44f993636553fecc297f2eb4df9f5ebe5a4acab6995c4b820df904386f7807bb589439b7").to_vec().try_into().unwrap(),
			hex_literal::hex!("3082032a308202b0a00302010202060191eaf64f46300a06082a8648ce3d040302304f3123302106035504030c1a4170706c6520417070204174746573746174696f6e204341203131133011060355040a0c0a4170706c6520496e632e3113301106035504080c0a43616c69666f726e6961301e170d3234303931323130333831365a170d3235303732343034323031365a3081913149304706035504030c4036383535336364646665363764323132313731616462323234353932393731613663393064346332653734613165386363323635373732396334303638616235311a3018060355040b0c114141412043657274696669636174696f6e31133011060355040a0c0a4170706c6520496e632e3113301106035504080c0a43616c69666f726e69613059301306072a8648ce3d020106082a8648ce3d03010703420004ed4b1e614c1b18ea36b940efd84c82fe661b136a80358946552d1338c78de0d43c6ddc9bd027718ea674a7dd4b7b9f1f2cdfc0840d61c9957d1ee57cd6800a1ca38201333082012f300c0603551d130101ff04023000300e0603551d0f0101ff0404030204f030818006092a864886f76364080504733071a40302010abf893003020101bf893103020100bf893203020101bf893303020101bf893421041f475632343532393232522e636f6d2e616375726173742e6578656375746f72a5060404736b7320bf893603020105bf893703020100bf893903020100bf893a03020100bf893b03020100305706092a864886f763640807044a3048bf8a7808040631372e362e31bf885007020500fffffffebf8a7b0704053231473933bf8a7d08040631372e362e31bf8a7e03020100bf8b0c0f040d32312e372e39332e302e302c30303306092a864886f76364080204263024a1220420e185a91f767e0dba7ea2a0fb76bc44a5a5b63537adc0097eada55f646d8c13f2300a06082a8648ce3d0403020368003065023100a8f61c00005ecac3e19bf6e2f2ddbc037682184d24140acc80be6b1476e12e4120a2dce0a530e690c4492c0c86511a53023011a4598c8a0e4d8f82ca5c04a06bf311bf56918a4364002655209eb12a6c4bed03b2e89b31a88f44d16daab8752e06e4").to_vec().try_into().unwrap(),
		].to_vec().try_into().unwrap();
		// within the leaf's validity, 2024-09-12 to 2025-07-24
		let validated = validate_certificate_chain(&chain, 1_735_689_600_000).map_err(|_| ())?;
		assert!(!validated.is_rkp);
		let cert = verified_leaf(&chain)?;
		let res = extract_attestation(&cert.extensions);
		match res {
			Ok(attestation) => match attestation {
				ParsedAttestation::DeviceAttestation(_) => Ok(()),
				_ => Err(()),
			},
			Err(_) => Err(()),
		}
	}

	#[test]
	fn test_validate_ios_chain_2() -> Result<(), ()> {
		let chain = [
			hex_literal::hex!("30820243308201c8a003020102021009bac5e1bc401ad9d45395bc381a0854300a06082a8648ce3d04030330523126302406035504030c1d4170706c6520417070204174746573746174696f6e20526f6f7420434131133011060355040a0c0a4170706c6520496e632e3113301106035504080c0a43616c69666f726e6961301e170d3230303331383138333935355a170d3330303331333030303030305a304f3123302106035504030c1a4170706c6520417070204174746573746174696f6e204341203131133011060355040a0c0a4170706c6520496e632e3113301106035504080c0a43616c69666f726e69613076301006072a8648ce3d020106052b8104002203620004ae5b37a0774d79b2358f40e7d1f22626f1c25fef17802deab3826a59874ff8d2ad1525789aa26604191248b63cb967069e98d363bd5e370fbfa08e329e8073a985e7746ea359a2f66f29db32af455e211658d567af9e267eb2614dc21a66ce99a366306430120603551d130101ff040830060101ff020100301f0603551d23041830168014ac91105333bdbe6841ffa70ca9e5faeae5e58aa1301d0603551d0e041604143ee35d1c0419a9c9b431f88474d6e1e15772e39b300e0603551d0f0101ff040403020106300a06082a8648ce3d0403030369003066023100bbbe888d738d0502cfbcfd666d09575035bcd6872c3f8430492629edd1f914e879991c9ae8b5aef8d3a85433f7b60d06023100ab38edd0cc81ed00a452c3ba44f993636553fecc297f2eb4df9f5ebe5a4acab6995c4b820df904386f7807bb589439b7").to_vec().try_into().unwrap(),
			hex_literal::hex!("3082036d308202f4a003020102020601928f7ff4be300a06082a8648ce3d040302304f3123302106035504030c1a4170706c6520417070204174746573746174696f6e204341203131133011060355040a0c0a4170706c6520496e632e3113301106035504080c0a43616c69666f726e6961301e170d3234313031343039323632305a170d3235303730393136343632305a3081913149304706035504030c4032353761636165393263656365333661343133666261646264633036306238323162646230656464386166366334376333373566323737633063663561373134311a3018060355040b0c114141412043657274696669636174696f6e31133011060355040a0c0a4170706c6520496e632e3113301106035504080c0a43616c69666f726e69613059301306072a8648ce3d020106082a8648ce3d030107034200042890afb5a9fd64f7c60a84bd5096b72f89ff9c794098281560ba72399cce7508ec3356edf4854add327d42e4b465bbffee15cd3c3dad152db0b0e2cfa7dd759aa382017730820173300c0603551d130101ff04023000300e0603551d0f0101ff0404030204f030818006092a864886f76364080504733071a40302010abf893003020101bf893103020100bf893203020101bf893303020101bf893421041f475632343532393232522e636f6d2e616375726173742e6578656375746f72a5060404736b7320bf893603020105bf893703020100bf893903020100bf893a03020100bf893b0302010030819a06092a864886f76364080704818c308189bf8a7808040631382e302e31bf885003020100bf8a7b09040732324133333730bf8a7c08040631382e302e31bf8a7d08040631382e302e31bf8a7e03020100bf8b0a10040e32322e312e3337302e332e302c30bf8b0b10040e32322e312e3337302e332e302c30bf8b0c10040e32322e312e3337302e332e302c30bf88020a04086970686f6e656f73303306092a864886f76364080204263024a1220420145ba64f1d8598845f480a2076e7224326b82642242ce39265feadf19a978ab8300a06082a8648ce3d040302036700306402304d4f5469770b138ed80ea73c9f73624eba5fc759ce1e346eaccd5d2b11bb4548875aed9fbacd6c973f8749bc6fba1b00023012fc340363c86353fdc568aa8b11706c2bb5cbb50b4fc21fcfa4dbce9f6b0b1aa5ad5906ec0c369845d6f5da93fa19b4").to_vec().try_into().unwrap(),
		].to_vec().try_into().unwrap();
		let cert = verified_leaf(&chain)?;
		let res = extract_attestation(&cert.extensions);
		match res {
			Ok(attestation) => match attestation {
				ParsedAttestation::DeviceAttestation(_) => Ok(()),
				_ => Err(()),
			},
			Err(_) => Err(()),
		}
	}

	#[test]
	fn test_validate_ios_chain_3() -> Result<(), ()> {
		let chain = [
			hex_literal::hex!("30820243308201c8a003020102021009bac5e1bc401ad9d45395bc381a0854300a06082a8648ce3d04030330523126302406035504030c1d4170706c6520417070204174746573746174696f6e20526f6f7420434131133011060355040a0c0a4170706c6520496e632e3113301106035504080c0a43616c69666f726e6961301e170d3230303331383138333935355a170d3330303331333030303030305a304f3123302106035504030c1a4170706c6520417070204174746573746174696f6e204341203131133011060355040a0c0a4170706c6520496e632e3113301106035504080c0a43616c69666f726e69613076301006072a8648ce3d020106052b8104002203620004ae5b37a0774d79b2358f40e7d1f22626f1c25fef17802deab3826a59874ff8d2ad1525789aa26604191248b63cb967069e98d363bd5e370fbfa08e329e8073a985e7746ea359a2f66f29db32af455e211658d567af9e267eb2614dc21a66ce99a366306430120603551d130101ff040830060101ff020100301f0603551d23041830168014ac91105333bdbe6841ffa70ca9e5faeae5e58aa1301d0603551d0e041604143ee35d1c0419a9c9b431f88474d6e1e15772e39b300e0603551d0f0101ff040403020106300a06082a8648ce3d0403030369003066023100bbbe888d738d0502cfbcfd666d09575035bcd6872c3f8430492629edd1f914e879991c9ae8b5aef8d3a85433f7b60d06023100ab38edd0cc81ed00a452c3ba44f993636553fecc297f2eb4df9f5ebe5a4acab6995c4b820df904386f7807bb589439b7").to_vec().try_into().unwrap(),
			hex_literal::hex!("30820355308202dba0030201020206019290222805300a06082a8648ce3d040302304f3123302106035504030c1a4170706c6520417070204174746573746174696f6e204341203131133011060355040a0c0a4170706c6520496e632e3113301106035504080c0a43616c69666f726e6961301e170d3234313031343132323333305a170d3235303530313230333633305a3081913149304706035504030c4033353936386337346362336332383133306433386531346132643537333031656632623762326636633164656638346661613462346331396231386631663665311a3018060355040b0c114141412043657274696669636174696f6e31133011060355040a0c0a4170706c6520496e632e3113301106035504080c0a43616c69666f726e69613059301306072a8648ce3d020106082a8648ce3d03010703420004fc74290d7305aed590a485b1fea4cb17d2cee3ef86cbd05a18f553e078025531d8cd16525ed4661c820fe6f0d0d5fa61785fff4019871a789e482d9ef8e5a678a382015e3082015a300c0603551d130101ff04023000300e0603551d0f0101ff0404030204f030818006092a864886f76364080504733071a40302010abf893003020101bf893103020100bf893203020101bf893303020101bf893421041f475632343532393232522e636f6d2e616375726173742e6578656375746f72a5060404736b7320bf893603020105bf893703020100bf893903020100bf893a03020100bf893b0302010030818106092a864886f76364080704743072bf8a7806040431382e31bf885007020500fffffffebf8a7b0a04083232423530363961bf8a7d06040431382e31bf8a7e03020100bf8b0a0f040d32322e322e36392e352e312c30bf8b0c0f040d32322e322e36392e352e312c30bf88020a04086970686f6e656f73bf880506040442657461303306092a864886f76364080204263024a12204200093e616846df01f7e252468e3e6b3c1b80958f98e3809e7bb8486c9c75017ae300a06082a8648ce3d040302036800306502310097ab23f82c210de06769ff97cf97b4ca5521c9bdbbb856b997ca17194c193f3a29222bb5d56de1028cb699ec5068e731023073246cecd83aba24c4c95ae2a5b83b556afbfbe920350397604c3d929f092c92c0f547abec82630815e8f32a5f6e2b94").to_vec().try_into().unwrap(),
		].to_vec().try_into().unwrap();
		let cert = verified_leaf(&chain)?;
		let res = extract_attestation(&cert.extensions);
		match res {
			Ok(attestation) => match attestation {
				ParsedAttestation::DeviceAttestation(_) => Ok(()),
				_ => Err(()),
			},
			Err(_) => Err(()),
		}
	}

	#[test]
	fn test_validate_ios_chain_4() -> Result<(), ()> {
		let chain = [
			hex_literal::hex!("30820243308201c8a003020102021009bac5e1bc401ad9d45395bc381a0854300a06082a8648ce3d04030330523126302406035504030c1d4170706c6520417070204174746573746174696f6e20526f6f7420434131133011060355040a0c0a4170706c6520496e632e3113301106035504080c0a43616c69666f726e6961301e170d3230303331383138333935355a170d3330303331333030303030305a304f3123302106035504030c1a4170706c6520417070204174746573746174696f6e204341203131133011060355040a0c0a4170706c6520496e632e3113301106035504080c0a43616c69666f726e69613076301006072a8648ce3d020106052b8104002203620004ae5b37a0774d79b2358f40e7d1f22626f1c25fef17802deab3826a59874ff8d2ad1525789aa26604191248b63cb967069e98d363bd5e370fbfa08e329e8073a985e7746ea359a2f66f29db32af455e211658d567af9e267eb2614dc21a66ce99a366306430120603551d130101ff040830060101ff020100301f0603551d23041830168014ac91105333bdbe6841ffa70ca9e5faeae5e58aa1301d0603551d0e041604143ee35d1c0419a9c9b431f88474d6e1e15772e39b300e0603551d0f0101ff040403020106300a06082a8648ce3d0403030369003066023100bbbe888d738d0502cfbcfd666d09575035bcd6872c3f8430492629edd1f914e879991c9ae8b5aef8d3a85433f7b60d06023100ab38edd0cc81ed00a452c3ba44f993636553fecc297f2eb4df9f5ebe5a4acab6995c4b820df904386f7807bb589439b7").to_vec().try_into().unwrap(),
			hex_literal::hex!("3082039f30820325a0030201020206019293e26592300a06082a8648ce3d040302304f3123302106035504030c1a4170706c6520417070204174746573746174696f6e204341203131133011060355040a0c0a4170706c6520496e632e3113301106035504080c0a43616c69666f726e6961301e170d3234313031353035353232305a170d3235303432323032313332305a3081913149304706035504030c4066303531373038353965633963333238346430663863636330356166323336643733376464373330666236333539323034653363353266623864353037383565311a3018060355040b0c114141412043657274696669636174696f6e31133011060355040a0c0a4170706c6520496e632e3113301106035504080c0a43616c69666f726e69613059301306072a8648ce3d020106082a8648ce3d030107034200044a6bd9ca256f2bda5ae1eef8fd6e4d997d1ad188902287d8fca2ac980bdc7a3819fa9e4742e7b17edbe2b1a04285f700ee5e9f58fc4d5e5c7e9f3253e0144816a38201a8308201a4300c0603551d130101ff04023000300e0603551d0f0101ff0404030204f030818006092a864886f76364080504733071a40302010abf893003020101bf893103020100bf893203020101bf893303020101bf893421041f475632343532393232522e636f6d2e616375726173742e6578656375746f72a5060404736b7320bf893603020105bf893703020100bf893903020100bf893a03020100bf893b030201003081cb06092a864886f7636408070481bd3081babf8a7808040631382e302e31bf885003020102bf8a7b09040732324133333730bf8a7c08040631382e302e31bf8a7d08040631382e302e31bf8a7e03020100bf8a7f03020100bf8b0003020100bf8b0103020100bf8b0203020100bf8b0303020100bf8b0403020101bf8b0503020100bf8b0a10040e32322e312e3337302e332e302c30bf8b0b10040e32322e312e3337302e332e302c30bf8b0c10040e32322e312e3337302e332e302c30bf88020a04086970686f6e656f73303306092a864886f76364080204263024a1220420ce35294bddf18492b2746df4cec1f75df47f3642848d1d294598fb85e6bedf12300a06082a8648ce3d040302036800306502303342d4ebbf6b9a93c7b85a0775e86026760705458f3c88621375deaf479c96da48ea55d8136887cf198c6a00f8b3e073023100c854609bbaf098564ee4cc38a0207c6dce7c252e0abbf8791d20849eac4db5a59ab77cd252f48e00b9b9d95b94a2936a").to_vec().try_into().unwrap(),
		].to_vec().try_into().unwrap();
		let cert = verified_leaf(&chain)?;
		let res = extract_attestation(&cert.extensions);
		match res {
			Ok(attestation) => match attestation {
				ParsedAttestation::DeviceAttestation(_) => Ok(()),
				_ => Err(()),
			},
			Err(_) => Err(()),
		}
	}

	#[test]
	fn test_ios_os_version() {
		let cert = parse_cert(&hex_literal::hex!("30820339308202bfa003020102020601a186f9321f300a06082a8648ce3d040302304f3123302106035504030c1a4170706c6520417070204174746573746174696f6e204341203131133011060355040a0c0a4170706c6520496e632e3113301106035504080c0a43616c69666f726e6961301e170d3236313032383039323430305a170d3236313033313039323430305a3081913149304706035504030c4036343537366535626662633036633033306263373961333835653365336233643831646338366339363862313466623631633132336363323462363261623730311a3018060355040b0c114141412043657274696669636174696f6e31133011060355040a0c0a4170706c6520496e632e3113301106035504080c0a43616c69666f726e69613059301306072a8648ce3d020106082a8648ce3d030107034200046390b00d417b0c5d1b419ee91c17896e3cc050f174bb8e3358ec78998ad9d9f5803c47f636bf13659ed6d9af07c3be4b91d6a6addd4a3422f997cbdd7e12aec1a38201423082013e300c0603551d130101ff04023000300e0603551d0f0101ff0404030204f030140603551d25040d300b06092a864886f76364041830819106092a864886f763640805048183308180a40302010abf893003020101bf893103020100bf893203020100bf893303020101bf893421041f475632343532393232522e636f6d2e616375726173742e6578656375746f72a506040420736b73bf893603020105bf893703020100bf893903020100bf893a03020100bf893b03020100aa03020100bf893c06020420736b73303f06092a864886f76364080704323030bf8a7808040631352e382e38bf885007020500ffffffffbf8a79090407312e302e323438bf8a7c020400bf8a7d020400303306092a864886f76364080204263024a1220420f69eec9cf53f20992d8fc5ae3925802c0d95f308dc1c8b3fd236ff4dadd7a498300a06082a8648ce3d04030203680030650230031b0ee06cf508f6bcedad2756f82cc001cd90fd3499b4c5e5361fc9c2e603286e2fe65702198939d15e62c00ae43282023100b9075774f424174ba70fe7627d72f03127c12b1ce748e27a01b78b1608e87472bed96901c40cc61d376a91422bf7e9f0")).unwrap();
		let content: BoundedAttestationContent =
			extract_attestation(&cert.tbs_certificate.extensions)
				.unwrap()
				.try_into()
				.unwrap();
		assert_eq!(content.platform(), 1);
		assert_eq!(content.os_version(), Some(vec![15, 8, 8]));
		assert_eq!(content.os_patch_level(), None);
	}

	#[test]
	fn test_validate_cert_chain() -> Result<(), ValidationError> {
		let decoded_chain = [
            hex_literal::hex!("3082051c30820304a003020102020900d50ff25ba3f2d6b3300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3139313132323230333735385a170d3334313131383230333735385a301b31193017060355040513106639323030396538353362366230343530820222300d06092a864886f70d01010105000382020f003082020a0282020100afb6c7822bb1a701ec2bb42e8bcc541663abef982f32c77f7531030c97524b1b5fe809fbc72aa9451f743cbd9a6f1335744aa55e77f6b6ac3535ee17c25e639517dd9c92e6374a53cbfe258f8ffbb6fd129378a22a4ca99c452d47a59f3201f44197ca1ccd7e762fb2f53151b6feb2fffd2b6fe4fe5bc6bd9ec34bfe08239daafceb8eb5a8ed2b3acd9c5e3a7790e1b51442793159859811ad9eb2a96bbdd7a57c93a91c41fccd27d67fd6f671aa0b815261ad384fa37944864604ddb3d8c4f920a19b1656c2f14ad6d03c56ec060899041c1ed1a5fe6d3440b556bad1d0a152589c53e55d370762f0122eef91861b1b0e6c4c80927499c0e9bec0b83e3bc1f93c72c049604bbd2f1345e62c3f8e26dbec06c94766f3c128239d4f4312fad8123887e06becf567583bf8355a81feeabaf99a83c8df3e2a322afc672bf120b135158b6821ceaf309b6eee77f98833b018daa10e451f06a374d50781f359082966bb778b9308942698e74e0bcd24628a01c2cc03e51f0b3e5b4ac1e4df9eaf9ff6a492a77c1483882885015b422ce67b80b88c9b48e13b607ab545c723ff8c44f8f2d368b9f6520d31145ebf9e862ad71df6a3bfd2450959d653740d97a12f368b13ef66d5d0a54a6e2f5d9a6fef446832bc67844725861f093dd0e6f3405da89643ef0f4d69b6420051fdb93049673e36950580d3cdf4fbd08bc58483952600630203010001a3633061301d0603551d0e041604143661e1007c880509518b446c47ff1a4cc9ea4f12301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff040403020204300d06092a864886f70d01010b050003820201004e31a05cf28ba65dbdafa1ced70969ee5ca84104added8a306cf7f6dee50375d745ed992cb0242cce72dc9eed51191fe5ad52bad7dd3b25c099e13a491a3cdd487a5acce8766324c4ae46338246ae7b78a418acbb98a05c4c9d696eeaab609d0ba0ce1a31be98490df3f4c0ea9ddc9e82ffb0fcb3e9ebdd8cb952789f2b1411fac56c886426eb7296042735da50e11ac715f1818cf9fdc4e254a3763351b6a2440150861263a6e310be1a50de5c7e8ee880fdd4be5884a37128d18830bb3476bf4291e82d5c66a6494939e08480bfbc00f7d8a74d43e73737ebe5d8e4ec515302d4689692780dc7538ed7e9175be6139e74d43ad388b3050ffd5a9de5262000898c01f63c53dfe22209108fa4f65ba16c49ccbde0837d7c5844d54b7398ba0122e505b155c9313cfe26e72d87e22aa1616e6bdbf547ddff93df29e35a63b455fe1fc0ec95581f3f4f7bbe3bb828396a37ae3157582bc3764b9780a239efc0f75a1e2e6d941ceabac27ddeb01e2bd8421029bea34d51aee6c60271d5a95ebd00515a9c0013dd80bf87eea260b81c34f688e6eb1348af0d8ea1cac32acb9d93fa24aff030a84c8f2b0f569cc95080b20ac35ace0c6d8dbd4f6847719519d32450166eb4bf15b859044501adeaf436382c34b15e3b54c92e61b69c2bfc7264589172b3c93dbe35ce06d08fd5c01322ca0877b1d12743af1fad5940ea1bc02dd891c").to_vec().try_into().unwrap(),
            hex_literal::hex!("308203943082017ca003020102021100cae43ccd6b2b17c68af41c889b236134300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3231303631363139313530335a170d3331303631343139313530335a3039310c300a060355040c0c03544545312930270603550405132034323638663430363630323561353736336264643264643639383339333361613076301006072a8648ce3d020106052b8104002203620004f592a71d74894920c279cfa5936817398891fa27f39dad1638178d5b3080ff324d2c117b65ac1e1e362f896c667197b81ce48a3e29aa922c2f0e068811cd5e109845afc735f9096158f4862b335e08ff92f4176ec30e066c9a53e785631e9516a3633061301d0603551d0e04160414887b508c220692460f5c4ab13a14acecd8795ec2301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff040403020204300d06092a864886f70d01010b0500038202010086cb38ea911920696b23283f7d07c0a27e9b9501d78c168a78b2a6a60002203b0f0bacfc7ad12122ab03c0325af4eda90148b95b2331e75484595a8fc380228f0fab15f5e7862ab24d0b29fc90c9a49d364cafda279385b0c1ec1a7a7e770d169d47e18b7ee28a6ef2d8a2f6239a6fdfa77f0822c67e58dbf860aec846c2c2c0be6080e2d161860b7715a8bd365520508361bbadda388007c25ea90c01864dfbbf17c6742e1ea4666b1c4c706bff105cd0034c87332904fc87fb51480d62c823930a9a19d68fce47a40f1b9cf367de9ff0e453229d358dd0e4e10658624f9d43861d075fb5afbdad36576962d00be006f14e969ab055139326930d9dd86a27163eff78a910f50a2a1a9e82f274081572f44eaa09d96f40b47cff42a95a545906fa5743523aeeff827c5f562b8c8e30e55b710427298b9d04e31b4be4527f285308f424f1c47093ce61623efb2f7283d9a6fd7afb64f537509b046a7fae3935bafc031842cbc72939e25d65304d5100a548f7ec54a86151d0d0a0b36eb5ca44cba682389451ef7405330fe65d2d83d3c977b79b024524430a75fe64acd189409602f6575ca090603433f084d3961dff4b93957e88b955f26b12683894d4a27132e4443927952d24395eb1fddcf37472a568766970bcadf5490162224ac72b65065de95d3532e5000baa72288072087625d8ee9128090f63cac7afbdf5f88919d7").to_vec().try_into().unwrap(),
            hex_literal::hex!("308201f330820179a003020102021073600bfb2b935c6b7cea5dab983e13f2300a06082a8648ce3d0403023039310c300a060355040c0c0354454531293027060355040513203432363866343036363032356135373633626464326464363938333933336161301e170d3231303631363139313830385a170d3331303631343139313830385a3039310c300a060355040c0c03544545312930270603550405132030323431353333343333353664366233373964616333313730666364613039313059301306072a8648ce3d020106082a8648ce3d03010703420004845c6ba85507ebbf1ff6d79c344bed87f7234246d37eb1d13c7089db57a3f527e9e1d099c83d291da16bdd5bb0514319d96992316aeef61cbf307de35c0fd787a3633061301d0603551d0e041604145b0a4a12838506b64a81e0efa4933685513e2523301f0603551d23041830168014887b508c220692460f5c4ab13a14acecd8795ec2300f0603551d130101ff040530030101ff300e0603551d0f0101ff040403020204300a06082a8648ce3d040302036800306502310084e9a4efd84cf528915f284fe6e9ba0804b219e8e971d03903f81e211547e15ae6e3e42e3d250e0c4dedc9d157a3b303023059e0da7c366a117985a9fe1bd465762813c558df1044f84b2cb159a1538b3e06e312ab793ba916a7c4bcec6f64ef4dd1").to_vec().try_into().unwrap(),
            hex_literal::hex!("308202b83082025ea003020102020101300a06082a8648ce3d0403023039310c300a060355040c0c0354454531293027060355040513203032343135333334333335366436623337396461633331373066636461303931301e170d3730303130313030303030305a170d3639313233313233353935395a301f311d301b06035504030c14416e64726f6964204b657973746f7265204b65793059301306072a8648ce3d020106082a8648ce3d030107034200045b05ac7b2ac523cc38de09340a70d2854006531bb007c3ad4d3231ceebd9a9663958c763ce36a00d0ccc15b2ff42f0e9775d9f411e3bcd15d41ad7ac94ca1a5ea382016f3082016b300e0603551d0f0101ff04040302078030820157060a2b06010401d67902011104820147308201430201030a01010201040a010104203ccc7f6af1f5a1b9e7cd2679d99eb249018dcd9f004d5a4f596621e8e7ad6e3b0400306abf853d08020601a039c0c1e8bf85455a045830563130302e0428636f6d2e616375726173742e61747465737465642e6578656375746f722e7362732e63616e6172790202008831220420ea21af13f3b724c662f3da05247acc5a68a45331a90220f0d90a6024d7fa8f363081a4a1083106020102020103a203020103a30402020100a5053103020100aa03020101bf8377020500bf853e03020100bf85404c304a0420d78769441545c2cd8a498d1446667b0bf9806b234535a5f52a602a20c9d53eea0101ff0a01000420e536f6d7b77e28900973d70ebac101e4e1a42109f7dd51f628af3a78c5e59af3bf854105020301d4c0bf85420502030316a8bf854e0602040134d9a1bf854f0602040134d9a1300a06082a8648ce3d0403020348003045022040b2eea88a263f7d1f2dde8e52d5b0dda0352559e901911bc935f189f2364175022100befb0c4996698df044818817731b4112253df177de36d0b6800fce7bdcdbf926").to_vec().try_into().unwrap(),
        ].to_vec().try_into().unwrap();
		let cert = verified_leaf(&decoded_chain)?;
		assert_eq!(cert.validity.not_before.timestamp_millis()?, 0);
		assert_eq!(cert.validity.not_after.timestamp_millis()?, 3_155_759_999_000);
		let parsed = extract_attestation(&cert.extensions)?;
		let bounded: BoundedAttestationContent = parsed.try_into().unwrap();
		assert_eq!(bounded.platform(), 0);
		assert_eq!(bounded.os_version(), Some(vec![12, 0, 0]));
		assert_eq!(bounded.os_patch_level(), Some(vec![2024, 8]));
		let BoundedAttestationContent::KeyDescription(kd) = bounded else { panic!() };
		let rot = kd.tee_enforced.root_of_trust.unwrap();
		assert_eq!(rot.verified_boot_state, VerifiedBootState::Verified);
		assert!(rot.device_locked);
		Ok(())
	}

	/// Synthetic per-schema parser tests. For each Android Keystore attestation
	/// schema version documented at
	/// https://source.android.com/docs/security/features/keystore/attestation
	/// we build a `KeyDescription` payload via test-only `Asn1Write` wire
	/// structs and verify our selective parser decodes it. No certificate chain
	/// validation here — these tests target the ASN.1 parser directly.
	mod schema_parse_tests {
		use asn1::{Asn1Write, Enumerated, Null};

		use crate::{
			attestation::asn::KeyDescription, BoundedAuthorizationList, BoundedKeyDescription,
			VerifiedBootState,
		};

		#[derive(Asn1Write)]
		struct RootOfTrustWire<'a> {
			verified_boot_key: &'a [u8],
			device_locked: bool,
			verified_boot_state: Enumerated,
			verified_boot_hash: &'a [u8],
		}

		#[derive(Asn1Write)]
		struct RootOfTrustV1V2Wire<'a> {
			verified_boot_key: &'a [u8],
			device_locked: bool,
			verified_boot_state: Enumerated,
		}

		/// Malformed: an unexpected 5th trailing field beyond the 4-field
		/// RootOfTrust. The parser must reject it, not silently drop it.
		#[derive(Asn1Write)]
		struct RootOfTrustMalformedWire<'a> {
			verified_boot_key: &'a [u8],
			device_locked: bool,
			verified_boot_state: Enumerated,
			verified_boot_hash: &'a [u8],
			unexpected_trailing: i64,
		}

		#[derive(Asn1Write)]
		struct AuthListRotOnlyWire<'a> {
			#[explicit(704)]
			root_of_trust: Option<RootOfTrustMalformedWire<'a>>,
		}

		#[derive(Asn1Write)]
		struct KeyDescriptionRotWire<'a> {
			attestation_version: i64,
			attestation_security_level: Enumerated,
			key_mint_version: i64,
			key_mint_security_level: Enumerated,
			attestation_challenge: &'a [u8],
			unique_id: &'a [u8],
			software_enforced: AuthListRotOnlyWire<'a>,
			tee_enforced: AuthListRotOnlyWire<'a>,
		}

		/// Wire-format `AuthorizationList` used to feed our selective parser.
		/// Fields must be declared in ascending tag order (DER requirement).
		/// Includes tags from the union of V1..V4..V100..V400 plus future tags
		/// 723 / 724 / 9999 that our parser should silently ignore.
		#[derive(Asn1Write, Default)]
		struct AuthorizationListWire<'a> {
			#[explicit(2)]
			algorithm: Option<i64>,
			#[explicit(3)]
			key_size: Option<i64>,
			#[explicit(10)]
			ec_curve: Option<i64>,
			#[explicit(200)]
			rsa_public_exponent: Option<i64>,
			#[explicit(303)]
			rollback_resistance: Option<Null>,
			#[explicit(305)]
			early_boot_only: Option<Null>,
			#[explicit(400)]
			active_date_time: Option<i64>,
			#[explicit(405)]
			usage_count_limit: Option<i64>,
			#[explicit(503)]
			no_auth_required: Option<Null>,
			#[explicit(506)]
			allow_while_on_body: Option<Null>,
			#[explicit(507)]
			trusted_user_presence_required: Option<Null>,
			#[explicit(508)]
			trusted_confirmation_required: Option<Null>,
			#[explicit(509)]
			unlocked_device_required: Option<Null>,
			#[explicit(701)]
			creation_date_time: Option<i64>,
			#[explicit(702)]
			origin: Option<i64>,
			#[explicit(704)]
			root_of_trust: Option<RootOfTrustWire<'a>>,
			#[explicit(705)]
			os_version: Option<i64>,
			#[explicit(706)]
			os_patch_level: Option<i64>,
			#[explicit(709)]
			attestation_application_id: Option<&'a [u8]>,
			#[explicit(710)]
			attestation_id_brand: Option<&'a [u8]>,
			#[explicit(718)]
			vendor_patch_level: Option<i64>,
			#[explicit(719)]
			boot_patch_level: Option<i64>,
			#[explicit(720)]
			device_unique_attestation: Option<Null>,
			/// Unknown to our parser (introduced in V200 — `attestationIdSecondImei`).
			#[explicit(723)]
			attestation_id_second_imei: Option<&'a [u8]>,
			/// Unknown to our parser (introduced in V400 — `moduleHash`).
			#[explicit(724)]
			module_hash: Option<&'a [u8]>,
			/// Future tag Google has not assigned. Forward-compat smoke field.
			#[explicit(9999)]
			future_unknown: Option<i64>,
		}

		/// V1/V2 variant whose `root_of_trust` is the 3-field shape. The rest
		/// of the layout matches V1/V2's restricted field set.
		#[derive(Asn1Write, Default)]
		struct AuthorizationListLegacyWire<'a> {
			#[explicit(2)]
			algorithm: Option<i64>,
			#[explicit(3)]
			key_size: Option<i64>,
			#[explicit(503)]
			no_auth_required: Option<Null>,
			#[explicit(704)]
			root_of_trust: Option<RootOfTrustV1V2Wire<'a>>,
			#[explicit(705)]
			os_version: Option<i64>,
			#[explicit(706)]
			os_patch_level: Option<i64>,
			#[explicit(709)]
			attestation_application_id: Option<&'a [u8]>,
			#[explicit(710)]
			attestation_id_brand: Option<&'a [u8]>,
		}

		#[derive(Asn1Write)]
		struct KeyDescriptionWire<'a> {
			attestation_version: i64,
			attestation_security_level: Enumerated,
			key_mint_version: i64,
			key_mint_security_level: Enumerated,
			attestation_challenge: &'a [u8],
			unique_id: &'a [u8],
			software_enforced: AuthorizationListWire<'a>,
			tee_enforced: AuthorizationListWire<'a>,
		}

		#[derive(Asn1Write)]
		struct KeyDescriptionLegacyWire<'a> {
			attestation_version: i64,
			attestation_security_level: Enumerated,
			key_mint_version: i64,
			key_mint_security_level: Enumerated,
			attestation_challenge: &'a [u8],
			unique_id: &'a [u8],
			software_enforced: AuthorizationListLegacyWire<'a>,
			tee_enforced: AuthorizationListLegacyWire<'a>,
		}

		const CHALLENGE: &[u8] = b"challenge";
		const UNIQUE_ID: &[u8] = b"uid";
		const BOOT_KEY: &[u8] = &[0u8; 32];
		const BOOT_HASH: &[u8] = &[0u8; 32];

		fn sec_level(v: u32) -> Enumerated {
			Enumerated::new(v)
		}

		fn parse_kd(bytes: &[u8]) -> BoundedKeyDescription {
			let kd = asn1::parse_single::<KeyDescription>(bytes).expect("parse KeyDescription");
			kd.try_into().expect("convert to BoundedKeyDescription")
		}

		#[test]
		fn test_parse_schema_v1() {
			let wire = KeyDescriptionLegacyWire {
				attestation_version: 1,
				attestation_security_level: sec_level(1),
				key_mint_version: 2,
				key_mint_security_level: sec_level(1),
				attestation_challenge: CHALLENGE,
				unique_id: UNIQUE_ID,
				software_enforced: AuthorizationListLegacyWire::default(),
				tee_enforced: AuthorizationListLegacyWire {
					algorithm: Some(3),
					key_size: Some(256),
					no_auth_required: Some(()),
					root_of_trust: Some(RootOfTrustV1V2Wire {
						verified_boot_key: BOOT_KEY,
						device_locked: true,
						verified_boot_state: Enumerated::new(0),
					}),
					..Default::default()
				},
			};
			let bytes = asn1::write_single(&wire).expect("write_single");
			let bkd = parse_kd(&bytes);
			assert_eq!(bkd.tee_enforced.algorithm, Some(3));
			assert_eq!(bkd.tee_enforced.key_size, Some(256));
			assert!(bkd.tee_enforced.no_auth_required);
			// V1/V2 carry the 3-field RootOfTrust on the wire (no verified_boot_hash);
			// it must be preserved, not dropped.
			let rot = bkd.tee_enforced.root_of_trust.expect("V1 RootOfTrust preserved");
			assert!(rot.device_locked);
			assert_eq!(rot.verified_boot_state, VerifiedBootState::Verified);
			assert!(rot.verified_boot_hash.is_none(), "V1 RootOfTrust has no verified_boot_hash");
		}

		#[test]
		fn test_parse_schema_v2() {
			let wire = KeyDescriptionLegacyWire {
				attestation_version: 2,
				attestation_security_level: sec_level(1),
				key_mint_version: 3,
				key_mint_security_level: sec_level(1),
				attestation_challenge: CHALLENGE,
				unique_id: UNIQUE_ID,
				software_enforced: AuthorizationListLegacyWire::default(),
				tee_enforced: AuthorizationListLegacyWire {
					algorithm: Some(3),
					key_size: Some(256),
					no_auth_required: Some(()),
					root_of_trust: Some(RootOfTrustV1V2Wire {
						verified_boot_key: BOOT_KEY,
						device_locked: true,
						verified_boot_state: Enumerated::new(0),
					}),
					os_version: Some(28),
					os_patch_level: Some(202405),
					// Skip `attestation_application_id`: the bounded conversion
					// re-parses these bytes as a SEQUENCE (`AttestationApplicationId`),
					// so we'd need a fully-formed nested SET-OF payload to use it.
					attestation_application_id: None,
					attestation_id_brand: Some(b"google"),
				},
			};
			let bytes = asn1::write_single(&wire).expect("write_single");
			let bkd = parse_kd(&bytes);
			assert_eq!(bkd.tee_enforced.os_version, Some(28));
			assert_eq!(bkd.tee_enforced.os_patch_level, Some(202405));
			assert!(bkd.tee_enforced.attestation_id_brand.is_some());
			let rot = bkd.tee_enforced.root_of_trust.expect("V2 RootOfTrust preserved");
			assert!(rot.device_locked);
			assert_eq!(rot.verified_boot_state, VerifiedBootState::Verified);
			assert!(rot.verified_boot_hash.is_none(), "V2 RootOfTrust has no verified_boot_hash");
		}

		#[test]
		fn test_parse_schema_rejects_malformed_root_of_trust() {
			let wire = KeyDescriptionRotWire {
				attestation_version: 3,
				attestation_security_level: sec_level(1),
				key_mint_version: 3,
				key_mint_security_level: sec_level(1),
				attestation_challenge: CHALLENGE,
				unique_id: UNIQUE_ID,
				software_enforced: AuthListRotOnlyWire { root_of_trust: None },
				tee_enforced: AuthListRotOnlyWire {
					root_of_trust: Some(RootOfTrustMalformedWire {
						verified_boot_key: BOOT_KEY,
						device_locked: true,
						verified_boot_state: Enumerated::new(0),
						verified_boot_hash: BOOT_HASH,
						unexpected_trailing: 1,
					}),
				},
			};
			let bytes = asn1::write_single(&wire).expect("write_single");
			assert!(
				asn1::parse_single::<KeyDescription>(&bytes).is_err(),
				"malformed RootOfTrust must reject, not silently drop"
			);
		}

		fn modern_authlist<'a>() -> AuthorizationListWire<'a> {
			AuthorizationListWire {
				algorithm: Some(3),
				key_size: Some(256),
				rollback_resistance: Some(()),
				no_auth_required: Some(()),
				root_of_trust: Some(RootOfTrustWire {
					verified_boot_key: BOOT_KEY,
					device_locked: true,
					verified_boot_state: Enumerated::new(0),
					verified_boot_hash: BOOT_HASH,
				}),
				os_version: Some(28),
				os_patch_level: Some(202405),
				vendor_patch_level: Some(202404),
				boot_patch_level: Some(202403),
				..Default::default()
			}
		}

		fn modern_kd(version: i64, tee_enforced: AuthorizationListWire) -> KeyDescriptionWire {
			KeyDescriptionWire {
				attestation_version: version,
				attestation_security_level: sec_level(1),
				key_mint_version: version,
				key_mint_security_level: sec_level(1),
				attestation_challenge: CHALLENGE,
				unique_id: UNIQUE_ID,
				software_enforced: AuthorizationListWire::default(),
				tee_enforced,
			}
		}

		fn assert_modern_baseline(bkd: &BoundedAuthorizationList) {
			assert_eq!(bkd.algorithm, Some(3));
			assert_eq!(bkd.key_size, Some(256));
			assert_eq!(bkd.rollback_resistance, Some(true));
			assert!(bkd.no_auth_required);
			assert!(bkd.root_of_trust.is_some());
			assert_eq!(bkd.os_version, Some(28));
			assert_eq!(bkd.os_patch_level, Some(202405));
			assert_eq!(bkd.vendor_patch_level, Some(202404));
			assert_eq!(bkd.boot_patch_level, Some(202403));
		}

		#[test]
		fn test_parse_schema_v3() {
			let bytes = asn1::write_single(&modern_kd(3, modern_authlist())).unwrap();
			let bkd = parse_kd(&bytes);
			assert_modern_baseline(&bkd.tee_enforced);
		}

		#[test]
		fn test_parse_schema_v4() {
			let mut al = modern_authlist();
			al.early_boot_only = Some(());
			al.device_unique_attestation = Some(());
			let bytes = asn1::write_single(&modern_kd(4, al)).unwrap();
			let bkd = parse_kd(&bytes);
			assert_modern_baseline(&bkd.tee_enforced);
			assert_eq!(bkd.tee_enforced.early_boot_only, Some(true));
			assert_eq!(bkd.tee_enforced.device_unique_attestation, Some(true));
		}

		#[test]
		fn test_parse_schema_v100() {
			let mut al = modern_authlist();
			al.early_boot_only = Some(());
			al.usage_count_limit = Some(7);
			let bytes = asn1::write_single(&modern_kd(100, al)).unwrap();
			let bkd = parse_kd(&bytes);
			assert_modern_baseline(&bkd.tee_enforced);
			assert_eq!(bkd.tee_enforced.usage_count_limit, Some(7));
		}

		#[test]
		fn test_parse_schema_v200() {
			let mut al = modern_authlist();
			al.usage_count_limit = Some(7);
			al.attestation_id_second_imei = Some(b"second-imei");
			let bytes = asn1::write_single(&modern_kd(200, al)).unwrap();
			let bkd = parse_kd(&bytes);
			assert_modern_baseline(&bkd.tee_enforced);
			assert_eq!(bkd.tee_enforced.usage_count_limit, Some(7));
		}

		#[test]
		fn test_parse_schema_v300() {
			let mut al = modern_authlist();
			al.usage_count_limit = Some(7);
			al.attestation_id_second_imei = Some(b"second-imei");
			let bytes = asn1::write_single(&modern_kd(300, al)).unwrap();
			let bkd = parse_kd(&bytes);
			assert_modern_baseline(&bkd.tee_enforced);
		}

		#[test]
		fn test_parse_schema_v400() {
			let mut al = modern_authlist();
			al.usage_count_limit = Some(7);
			al.attestation_id_second_imei = Some(b"second-imei");
			al.module_hash = Some(b"module-hash-bytes");
			let bytes = asn1::write_single(&modern_kd(400, al)).unwrap();
			let bkd = parse_kd(&bytes);
			assert_modern_baseline(&bkd.tee_enforced);
		}

		#[test]
		fn test_parse_schema_with_unknown_future_tag() {
			let mut al = modern_authlist();
			al.usage_count_limit = Some(7);
			al.module_hash = Some(b"module-hash");
			al.future_unknown = Some(0x1234);
			let bytes = asn1::write_single(&modern_kd(400, al)).unwrap();
			let bkd = parse_kd(&bytes);
			assert_modern_baseline(&bkd.tee_enforced);
			assert_eq!(bkd.tee_enforced.usage_count_limit, Some(7));
		}
	}
}

#[cfg(test)]
mod android_chain_tests {
	use super::*;

	/// A mainnet onboarding chain, root first, one hex certificate per line.
	fn chain(text: &str) -> CertificateChainInput {
		let certs = text
			.lines()
			.map(|line| {
				CertificateInput::truncate_from(impl_serde::serialize::from_hex(line).unwrap())
			})
			.collect::<Vec<_>>();
		CertificateChainInput::truncate_from(certs)
	}

	fn rkp_genuine() -> CertificateChainInput {
		chain(include_str!("./__test_chains__/rkp_genuine.txt"))
	}

	fn keybox() -> CertificateChainInput {
		chain(include_str!("./__test_chains__/keybox.txt"))
	}

	/// Onboarding times of the fixtures, in unix millis.
	const RKP_GENUINE_ONBOARDED: u64 = 1_789_323_732_000;
	const KEYBOX_ONBOARDED: u64 = 1_789_323_744_000;

	/// Whether [validate_certificate_chain] accepts `chain` at `now` as an RKP chain.
	fn is_rkp(chain: &CertificateChainInput, now: u64) -> Result<bool, ValidationError> {
		validate_certificate_chain(chain, now).map(|chain| chain.is_rkp)
	}

	fn without_root(chain: CertificateChainInput) -> CertificateChainInput {
		CertificateChainInput::truncate_from(chain[1..].to_vec())
	}

	/// The certificates of `chain` at `indices`, in that order.
	fn pick(chain: &CertificateChainInput, indices: &[usize]) -> CertificateChainInput {
		CertificateChainInput::truncate_from(indices.iter().map(|i| chain[*i].clone()).collect())
	}

	#[test]
	fn genuine_chains_are_accepted() {
		for (chain, now, expected) in [
			(rkp_genuine(), RKP_GENUINE_ONBOARDED, true),
			(without_root(rkp_genuine()), RKP_GENUINE_ONBOARDED, true),
			(keybox(), KEYBOX_ONBOARDED, false),
			(without_root(keybox()), KEYBOX_ONBOARDED, false),
		] {
			assert_eq!(is_rkp(&chain, now), Ok(expected));
		}
	}

	#[test]
	fn certificate_outside_its_validity_is_rejected() {
		// the RKP certificate of the fixture is valid from 2026-09-11T09:15:20Z to 2026-09-26T06:19:40Z
		let chain = rkp_genuine();
		assert_eq!(is_rkp(&chain, 1_790_403_579_999), Ok(true));
		for now in [1_789_118_119_999, 1_790_403_580_000] {
			assert_eq!(is_rkp(&chain, now), Err(ValidationError::CertificateNotValidAtTime));
		}
	}

	#[test]
	fn rkp_intermediates_are_not_classified_as_keybox() {
		// keybox length without the provisioning info, but issued by Droid CA2 instead of a factory intermediate
		let chain = pick(&rkp_genuine(), &[0, 1, 2, 4]);
		assert_eq!(
			chain_policy(&chain, RKP_GENUINE_ONBOARDED),
			Err(ValidationError::UnexpectedChainShape)
		);
	}

	#[test]
	fn rkp_chain_length_is_not_fixed() {
		// as if Google dropped or added an RKP intermediate
		for indices in [&[0, 3, 4][..], &[0, 1, 1, 2, 3, 4]] {
			let chain = pick(&rkp_genuine(), indices);
			assert_eq!(chain_policy(&chain, RKP_GENUINE_ONBOARDED), Ok(true));
		}
	}

	#[test]
	fn repeated_trusted_root_is_rejected() {
		// a second root would otherwise be read as the certificate the root issued
		let chain = pick(&keybox(), &[0, 0, 2, 3]);
		assert_eq!(validate_chain(&chain), Err(ValidationError::UnexpectedChainShape));
	}

	#[test]
	fn rkp_chain_cannot_pass_as_keybox() {
		let rkp = rkp_genuine();
		// every proper subchain that keeps the attested leaf
		for mask in (1u32 << (rkp.len() - 1))..(1u32 << rkp.len()) - 1 {
			let indices: Vec<usize> = (0..rkp.len()).filter(|i| mask & (1 << i) != 0).collect();
			let chain = pick(&rkp, &indices);
			assert_ne!(
				is_rkp(&chain, RKP_GENUINE_ONBOARDED),
				Ok(false),
				"subchain {mask:05b} passes as keybox"
			);
		}
	}

	#[test]
	fn keybox_intermediate_expiry_is_ignored() {
		// the intermediates of the fixture are valid from 2020-01-07 to 2030-01-04
		for chain in [keybox(), without_root(keybox())] {
			assert_eq!(chain_policy(&chain, 1_900_000_000_000), Ok(false));
			assert_eq!(
				chain_policy(&chain, 1_550_000_000_000),
				Err(ValidationError::CertificateNotValidAtTime)
			);
		}
	}

	/// A mainnet onboarding whose StrongBox key is signed by an RKP device certificate claiming TEE.
	#[test]
	fn strongbox_key_under_tee_rkp_certificate_is_rejected() {
		let chain = chain(include_str!("./__test_chains__/strongbox_key_under_tee.txt"));
		assert_eq!(is_rkp(&chain, 1_792_392_528_000), Err(ValidationError::SecurityLevelMismatch));
	}

	#[test]
	fn not_approved_build_is_rejected() {
		let chain = chain(include_str!("./__test_chains__/not_approved.txt"));
		assert_eq!(is_rkp(&chain, 1_789_406_952_000), Err(ValidationError::BuildNotApproved));
	}

	#[test]
	fn too_many_certificates_issued_is_rejected() {
		let chain = chain(include_str!("./__test_chains__/high_velocity.txt"));
		assert_eq!(
			is_rkp(&chain, 1_789_576_644_000),
			Err(ValidationError::TooManyCertificatesIssued)
		);
	}

	#[test]
	fn attest_key_chain_is_rejected() {
		let chain = chain(include_str!("./__test_chains__/attest_key.txt"));
		assert!(validate_chain(&chain).is_ok());
		assert_eq!(
			chain_policy(&chain, 1_791_575_034_000),
			Err(ValidationError::UnexpectedLeafSerialNumber)
		);
	}

	#[test]
	fn unexpected_shapes_are_rejected() {
		let rkp = rkp_genuine();
		let keybox = keybox();

		for (chain, now) in [
			// an extra certificate below the RKP certificate
			(pick(&rkp, &[0, 1, 2, 3, 3, 4]), RKP_GENUINE_ONBOARDED),
			// the same without the root
			(pick(&rkp, &[1, 2, 3, 3, 4]), RKP_GENUINE_ONBOARDED),
			// the RKP certificate not directly above the leaf
			(pick(&rkp, &[0, 1, 3, 2, 4]), RKP_GENUINE_ONBOARDED),
			// a keybox chain with an extra intermediate
			(pick(&keybox, &[0, 1, 2, 2, 3]), KEYBOX_ONBOARDED),
			// the same without the root, as long as a genuine keybox chain with the root
			(pick(&keybox, &[1, 2, 2, 3]), KEYBOX_ONBOARDED),
		] {
			assert_eq!(chain_policy(&chain, now), Err(ValidationError::UnexpectedChainShape));
		}
	}

	#[test]
	fn leaf_serial_other_than_one_is_rejected() {
		let keybox = keybox();
		let mut leaf = keybox[3].to_vec();
		assert_eq!(parse_cert(&leaf).unwrap().tbs_certificate.serial_number.as_bytes(), [1]);
		// the serial INTEGER `02 01 01` is the first one in the leaf; patch its value
		let at = leaf.windows(3).position(|w| w == [0x02, 0x01, 0x01]).unwrap();
		leaf[at + 2] = 2;
		let chain = CertificateChainInput::truncate_from(vec![
			keybox[0].clone(),
			keybox[1].clone(),
			keybox[2].clone(),
			CertificateInput::truncate_from(leaf),
		]);
		assert_eq!(
			chain_policy(&chain, KEYBOX_ONBOARDED),
			Err(ValidationError::UnexpectedLeafSerialNumber)
		);
	}

	#[test]
	fn provisioning_info_is_parsed() {
		// {1: 64, 3: "x", 5: {1: "APPROVED", 2: "00ff"}}
		let info = hex_literal::hex!("a301184003617805a20168415050524f564544" "026430306666");
		assert_eq!(
			ProvisioningInfo::parse(&info),
			Some(ProvisioningInfo {
				certificates_issued: Some(64),
				approval_status: Some(b"APPROVED".as_slice()),
				vbmeta_digest: Some(b"00ff".as_slice()),
			})
		);
	}

	#[test]
	fn missing_approval_status_is_rejected() {
		// {1: 64}, {1: 64, 5: {}} and {1: 64, 5: {2: h'00'}}
		for info in [
			hex_literal::hex!("a1011840").as_slice(),
			hex_literal::hex!("a2011840" "05a0").as_slice(),
			hex_literal::hex!("a2011840" "05a1024100").as_slice(),
		] {
			assert_eq!(
				ensure_provisioning_info_accepted(info, None),
				Err(ValidationError::BuildNotApproved)
			);
		}
		// {1: 64, 5: {1: "NOT_APPROVED" | "PENDING"}}
		for status in [b"NOT_APPROVED".as_slice(), b"PENDING"] {
			let mut info = hex_literal::hex!("a2011840" "05a101").to_vec();
			info.push(0x60 + status.len() as u8);
			info.extend_from_slice(status);
			assert_eq!(
				ensure_provisioning_info_accepted(&info, None),
				Err(ValidationError::BuildNotApproved)
			);
		}
	}

	#[test]
	fn vbmeta_digest_must_match_the_leaf() {
		// {1: 64, 5: {1: "APPROVED", 2: "00fF"}}
		let info = hex_literal::hex!("a2011840" "05a20168415050524f564544" "026430306646");
		assert_eq!(ensure_provisioning_info_accepted(&info, Some(&[0x00, 0xff])), Ok(()));
		for leaf_hash in [None, Some([0x00, 0xfe].as_slice())] {
			assert_eq!(
				ensure_provisioning_info_accepted(&info, leaf_hash),
				Err(ValidationError::VerifiedBootHashMismatch)
			);
		}
		// {1: 64, 5: {1: "APPROVED", 2: "0x00ff"}}
		let prefixed = hex_literal::hex!("a2011840" "05a20168415050524f564544" "0266307830306666");
		assert_eq!(
			ensure_provisioning_info_accepted(&prefixed, Some(&[0x00, 0xff])),
			Err(ValidationError::VerifiedBootHashMismatch)
		);
		// {1: 64, 5: {1: "APPROVED"}}
		let without_digest = hex_literal::hex!("a2011840" "05a10168415050524f564544");
		assert_eq!(
			ensure_provisioning_info_accepted(&without_digest, Some(&[0x00, 0xff])),
			Err(ValidationError::VerifiedBootHashMismatch)
		);
	}

	#[test]
	fn missing_certificates_issued_is_rejected() {
		// {5: {1: "APPROVED"}} and {1: "x", 5: {1: "APPROVED"}}
		for info in [
			hex_literal::hex!("a1" "05a10168415050524f564544").as_slice(),
			hex_literal::hex!("a2016178" "05a10168415050524f564544").as_slice(),
		] {
			assert_eq!(
				ensure_provisioning_info_accepted(info, None),
				Err(ValidationError::TooManyCertificatesIssued)
			);
		}
	}

	#[test]
	fn malformed_provisioning_info_is_rejected() {
		// truncated map
		assert_eq!(ProvisioningInfo::parse(&hex_literal::hex!("a20118")), None);
		// not a map
		assert_eq!(ProvisioningInfo::parse(&hex_literal::hex!("01")), None);
		// indefinite length map
		assert_eq!(ProvisioningInfo::parse(&hex_literal::hex!("bf0101ff")), None);
		// trailing bytes
		assert_eq!(ProvisioningInfo::parse(&hex_literal::hex!("a1010100")), None);
		// text running past the end
		assert_eq!(ProvisioningInfo::parse(&hex_literal::hex!("a105a1016841")), None);
		// nesting deeper than allowed
		let mut deep = vec![0xa1, 0x02];
		deep.extend([0x81; 16]);
		deep.push(0x00);
		assert_eq!(ProvisioningInfo::parse(&deep), None);
	}
}
