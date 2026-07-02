use core::{
	convert::TryInto,
	hash::{Hash, Hasher},
};

use asn1::{
	parse, Asn1Read, Asn1Readable, Asn1Writable, Asn1Write, BitString, Enumerated, Explicit, Null,
	ObjectIdentifier, ParseResult, SequenceOf, SetOf, SimpleAsn1Readable, SimpleAsn1Writable, Tag,
	Tlv, WriteBuf, WriteResult,
};
use chrono::{self, Datelike, Timelike};
use sp_std::prelude::*;

#[derive(Asn1Read, Asn1Write, Clone)]
/// Represents the root structure of a [X.509 v3 certificate](https://www.rfc-editor.org/rfc/rfc5280#section-4.1)
/// See how to map these to [asn1 structs](https://docs.rs/asn1/0.11.0/asn1/#structs)
pub struct Certificate<'a> {
	// https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html
	pub tbs_certificate: TBSCertificate<'a>,
	pub signature_algorithm: AlgorithmIdentifier<'a>,
	pub signature_value: BitString<'a>,
}

#[derive(Asn1Read, Asn1Write)]
/// As Certificate, represents the root structure of a [X.509 v3 certificate](https://www.rfc-editor.org/rfc/rfc5280#section-4.1).
/// This version does not decode the payload.
/// See how to map these to [asn1 structs](https://docs.rs/asn1/0.11.0/asn1/#structs)
pub struct CertificateRawPayload<'a> {
	// https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html
	pub tbs_certificate: Tlv<'a>,
	pub signature_algorithm: AlgorithmIdentifier<'a>,
	pub signature_value: BitString<'a>,
}

#[derive(Asn1Read, Asn1Write, Clone)]
/// [See RFC](https://www.rfc-editor.org/rfc/rfc5280#section-4.1.1.2)
pub struct AlgorithmIdentifier<'a> {
	pub algorithm: ObjectIdentifier,
	pub parameters: Option<Tlv<'a>>,
}

#[derive(Asn1Read, Asn1Write, Clone)]
pub struct TBSCertificate<'a> {
	#[explicit(0)]
	#[default(1u64)]
	pub version: u64,
	pub serial_number: asn1::BigUint<'a>,
	pub signature: AlgorithmIdentifier<'a>,
	// RFC: https://www.rfc-editor.org/rfc/rfc5280#section-4.1.2.4
	pub issuer: Name<'a>,
	pub validity: Validity,
	pub subject: Name<'a>,
	pub subject_public_key_info: SubjectPublicKeyInfo<'a>,
	// If present, version MUST be v2 or v3
	#[implicit(1)]
	pub issuer_unique_id: Option<BitString<'a>>,
	// If present, version MUST be v2 or v3
	#[implicit(2)]
	pub subject_unique_id: Option<BitString<'a>>,
	// If present, version MUST be v3
	#[explicit(3)]
	pub extensions: Option<SequenceOf<'a, Extension<'a>>>,
}

#[derive(asn1::Asn1Read, asn1::Asn1Write, Clone)]
pub enum Name<'a> {
	RDNSequence(RDNSequence<'a>),
}

type RDNSequence<'a> = SequenceOf<'a, RelativeDistinguishedName<'a>>;

type RelativeDistinguishedName<'a> = SetOf<'a, AttributeTypeAndValue<'a>>;

#[derive(Asn1Read, Asn1Write)]
pub struct AttributeTypeAndValue<'a> {
	pub typ: ObjectIdentifier,
	/// A value with the format defined by `typ`.
	/// See https://www.rfc-editor.org/rfc/rfc5280#section-4.1.2.4
	pub value: Tlv<'a>,
}

#[derive(Asn1Read, Asn1Write, Clone)]
pub struct Validity {
	pub not_before: Time,
	pub not_after: Time,
}

#[derive(Asn1Read, Asn1Write, Clone)]
pub enum Time {
	UTCTime(asn1::UtcTime),
	GeneralizedTime(asn1::GeneralizedTime),
}

impl Time {
	pub fn timestamp_millis(&self) -> u64 {
		let date_time = match self {
			Time::UTCTime(time) => time.as_datetime(), //time.as_chrono().timestamp_millis().try_into().unwrap(),
			Time::GeneralizedTime(time) => time.as_datetime(), //time.as_chrono().timestamp_millis().try_into().unwrap(),
		};
		let initial = chrono::NaiveDateTime::default();
		let milliseconds = initial
			.with_second(date_time.second().into())
			.and_then(|t| {
				t.with_minute(date_time.minute().into()).and_then(|t| {
					t.with_hour(date_time.hour().into()).and_then(|t| {
						t.with_day(date_time.day().into()).and_then(|t| {
							t.with_month(date_time.month().into())
								.and_then(|t| t.with_year(date_time.year().into()))
						})
					})
				})
			})
			.map(|t| t.and_utc().timestamp_millis())
			.unwrap_or(0);

		milliseconds.try_into().unwrap()
	}
}

#[derive(Asn1Read, Asn1Write, Clone)]
pub struct SubjectPublicKeyInfo<'a> {
	pub algorithm: AlgorithmIdentifier<'a>,
	pub subject_public_key: BitString<'a>,
}

#[derive(Asn1Write, Clone)]
pub struct Extension<'a> {
	pub extn_id: ObjectIdentifier,
	#[default(false)]
	pub critical: bool,
	/// contains the DER encoding of an ASN.1 value
	/// corresponding to the extension type identified by extnID
	pub extn_value: &'a [u8],
}

impl<'a> SimpleAsn1Readable<'a> for Extension<'a> {
	const TAG: Tag = <asn1::Sequence as SimpleAsn1Readable>::TAG;

	fn parse_data(data: &'a [u8]) -> ParseResult<Self> {
		asn1::parse(data, |p| {
			let extn_id = p.read_element::<ObjectIdentifier>()?;
			let mut critical = false;

			let mut tlv = p.read_element::<Tlv>()?;
			if tlv.tag() == <bool as SimpleAsn1Readable>::TAG {
				critical = read_lenient_boolean(&tlv)?;
				tlv = p.read_element()?;
			}

			if tlv.tag() != <&[u8] as SimpleAsn1Readable>::TAG {
				return Err(asn1::ParseError::new(asn1::ParseErrorKind::InvalidValue));
			}

			let extn_value = tlv.data();

			Ok(Self { extn_id, critical, extn_value })
		})
	}
}

fn read_lenient_boolean(tlv: &Tlv<'_>) -> ParseResult<bool> {
	let data = tlv.data();
	if data.len() != 1 {
		return Err(asn1::ParseError::new(asn1::ParseErrorKind::InvalidValue));
	}
	Ok(data[0] != 0) // accept 0xFF (DER) or 0x01 (non-canonical BER)
}

/// Android KeyDescription. Outer layout is identical across all schema versions
/// (V1..V4, KeyMint V100/V200/V300/V400); only the inner `AuthorizationList`
/// differs. `AuthorizationList` is decoded selectively (tag-whitelist) so unknown
/// fields and new schema versions do not break parsing.
#[derive(Asn1Read)]
pub struct KeyDescription<'a> {
	pub attestation_version: i64,
	pub attestation_security_level: SecurityLevel,
	pub key_mint_version: i64,
	pub key_mint_security_level: SecurityLevel,
	pub attestation_challenge: &'a [u8],
	pub unique_id: &'a [u8],
	pub software_enforced: AuthorizationList<'a>,
	pub tee_enforced: AuthorizationList<'a>,
}

fn try_parse_tags<'a>(
	parser: &mut asn1::Parser<'a>,
	tags: &[asn1::Tag],
) -> ParseResult<Vec<asn1::Tlv<'a>>> {
	let mut result = vec![];
	while !parser.is_empty() {
		let tlv = parser.read_element::<asn1::Tlv>()?;
		if tags.contains(&tlv.tag()) {
			result.push(tlv);
		}
	}
	Ok(result)
}

pub struct DeviceAttestationKeyUsageProperties<'a> {
	pub t4: Option<i64>,
	pub t1200: Option<i64>,
	pub t1201: Option<i64>,
	pub t1202: Option<i64>,
	pub t1203: Option<i64>,
	pub t1204: Option<&'a [u8]>,
	pub t5: Option<&'a [u8]>,
	pub t1206: Option<i64>,
	pub t1207: Option<i64>,
	pub t1209: Option<i64>,
	pub t1210: Option<i64>,
	pub t1211: Option<i64>,
}

impl<'a> SimpleAsn1Readable<'a> for DeviceAttestationKeyUsageProperties<'a> {
	const TAG: Tag = <asn1::Sequence as SimpleAsn1Readable>::TAG;

	fn parse_data(data: &'a [u8]) -> ParseResult<Self> {
		asn1::parse(data, |parser| {
			let t4 = Tag::from_bytes(&[0xA4, 0x03])?.0;
			let t1200 = Tag::from_bytes(&[0xBF, 0x89, 0x30, 0x03])?.0;
			let t1201 = Tag::from_bytes(&[0xBF, 0x89, 0x31, 0x03])?.0;
			let t1202 = Tag::from_bytes(&[0xBF, 0x89, 0x32, 0x03])?.0;
			let t1203 = Tag::from_bytes(&[0xBF, 0x89, 0x33, 0x03])?.0;
			let t1204 = Tag::from_bytes(&[0xBF, 0x89, 0x34, 0x21])?.0;
			let t5 = Tag::from_bytes(&[0xA5, 0x06])?.0;
			let t1206 = Tag::from_bytes(&[0xBF, 0x89, 0x36, 0x03])?.0;
			let t1207 = Tag::from_bytes(&[0xBF, 0x89, 0x37, 0x03])?.0;
			let t1209 = Tag::from_bytes(&[0xBF, 0x89, 0x39, 0x03])?.0;
			let t1210 = Tag::from_bytes(&[0xBF, 0x89, 0x3A, 0x03])?.0;
			let t1211 = Tag::from_bytes(&[0xBF, 0x89, 0x3B, 0x03])?.0;
			let tlvs = try_parse_tags(
				parser,
				&[t4, t1200, t1201, t1202, t1203, t1204, t5, t1206, t1207, t1209, t1210, t1211],
			)?;
			Ok(Self {
				t4: tlvs
					.iter()
					.find(|tlv| tlv.tag() == t4)
					.map(Tlv::parse::<Explicit<'a, _, 4>>)
					.transpose()?
					.map(Explicit::into_inner),
				t1200: tlvs
					.iter()
					.find(|tlv| tlv.tag() == t1200)
					.map(Tlv::parse::<Explicit<'a, _, 1200>>)
					.transpose()?
					.map(Explicit::into_inner),
				t1201: tlvs
					.iter()
					.find(|tlv| tlv.tag() == t1201)
					.map(Tlv::parse::<Explicit<'a, _, 1201>>)
					.transpose()?
					.map(Explicit::into_inner),
				t1202: tlvs
					.iter()
					.find(|tlv| tlv.tag() == t1202)
					.map(Tlv::parse::<Explicit<'a, _, 1202>>)
					.transpose()?
					.map(Explicit::into_inner),
				t1203: tlvs
					.iter()
					.find(|tlv| tlv.tag() == t1203)
					.map(Tlv::parse::<Explicit<'a, _, 1203>>)
					.transpose()?
					.map(Explicit::into_inner),
				t1204: tlvs
					.iter()
					.find(|tlv| tlv.tag() == t1204)
					.map(Tlv::parse::<Explicit<'a, _, 1204>>)
					.transpose()?
					.map(Explicit::into_inner),
				t5: tlvs
					.iter()
					.find(|tlv| tlv.tag() == t5)
					.map(Tlv::parse::<Explicit<'a, _, 5>>)
					.transpose()?
					.map(Explicit::into_inner),
				t1206: tlvs
					.iter()
					.find(|tlv| tlv.tag() == t1206)
					.map(Tlv::parse::<Explicit<'a, _, 1206>>)
					.transpose()?
					.map(Explicit::into_inner),
				t1207: tlvs
					.iter()
					.find(|tlv| tlv.tag() == t1207)
					.map(Tlv::parse::<Explicit<'a, _, 1207>>)
					.transpose()?
					.map(Explicit::into_inner),
				t1209: tlvs
					.iter()
					.find(|tlv| tlv.tag() == t1209)
					.map(Tlv::parse::<Explicit<'a, _, 1209>>)
					.transpose()?
					.map(Explicit::into_inner),
				t1210: tlvs
					.iter()
					.find(|tlv| tlv.tag() == t1210)
					.map(Tlv::parse::<Explicit<'a, _, 1210>>)
					.transpose()?
					.map(Explicit::into_inner),
				t1211: tlvs
					.iter()
					.find(|tlv| tlv.tag() == t1211)
					.map(Tlv::parse::<Explicit<'a, _, 1211>>)
					.transpose()?
					.map(Explicit::into_inner),
			})
		})
	}
}

pub struct DeviceAttestationDeviceOSInformation<'a> {
	pub t1400: Option<&'a [u8]>,
	pub t1104: Option<i64>,
	pub t1403: Option<&'a [u8]>,
	pub t1420: Option<&'a [u8]>,
	pub t1026: Option<&'a [u8]>,
	pub t1029: Option<&'a [u8]>,
}

impl<'a> SimpleAsn1Readable<'a> for DeviceAttestationDeviceOSInformation<'a> {
	const TAG: Tag = <asn1::Sequence as SimpleAsn1Readable>::TAG;

	fn parse_data(data: &'a [u8]) -> ParseResult<Self> {
		asn1::parse(data, |parser| {
			let t1400 = Tag::from_bytes(&[0xBF, 0x8A, 0x78, 0x08])?.0;
			let t1104 = Tag::from_bytes(&[0xBF, 0x88, 0x50, 0x03])?.0;
			let t1403 = Tag::from_bytes(&[0xBF, 0x8A, 0x7B, 0x09])?.0;
			let t1420 = Tag::from_bytes(&[0xBF, 0x8B, 0x0C, 0x10])?.0;
			let t1026 = Tag::from_bytes(&[0xBF, 0x88, 0x02, 0x0A])?.0;
			let t1029 = Tag::from_bytes(&[0xBF, 0x88, 0x05, 0x06])?.0;
			let tlvs = try_parse_tags(parser, &[t1400, t1104, t1403, t1420, t1026, t1029])?;
			Ok(Self {
				t1400: tlvs
					.iter()
					.find(|tlv| tlv.tag() == t1400)
					.map(Tlv::parse::<Explicit<'a, _, 1400>>)
					.transpose()?
					.map(Explicit::into_inner),
				t1104: tlvs
					.iter()
					.find(|tlv| tlv.tag() == t1104)
					.map(Tlv::parse::<Explicit<'a, _, 1104>>)
					.transpose()?
					.map(Explicit::into_inner),
				t1403: tlvs
					.iter()
					.find(|tlv| tlv.tag() == t1403)
					.map(Tlv::parse::<Explicit<'a, _, 1403>>)
					.transpose()?
					.map(Explicit::into_inner),
				t1420: tlvs
					.iter()
					.find(|tlv| tlv.tag() == t1420)
					.map(Tlv::parse::<Explicit<'a, _, 1420>>)
					.transpose()?
					.map(Explicit::into_inner),
				t1026: tlvs
					.iter()
					.find(|tlv| tlv.tag() == t1026)
					.map(Tlv::parse::<Explicit<'a, _, 1026>>)
					.transpose()?
					.map(Explicit::into_inner),
				t1029: tlvs
					.iter()
					.find(|tlv| tlv.tag() == t1029)
					.map(Tlv::parse::<Explicit<'a, _, 1029>>)
					.transpose()?
					.map(Explicit::into_inner),
			})
		})
	}
}

pub struct DeviceAttestationNonce<'a> {
	pub nonce: Option<&'a [u8]>,
}

impl<'a> SimpleAsn1Readable<'a> for DeviceAttestationNonce<'a> {
	const TAG: Tag = <asn1::Sequence as SimpleAsn1Readable>::TAG;

	fn parse_data(data: &'a [u8]) -> ParseResult<Self> {
		asn1::parse(data, |parser| {
			let nonce = Tag::from_bytes(&[0xA1, 0x22])?.0;
			let tlvs = try_parse_tags(parser, &[nonce])?;
			Ok(Self {
				nonce: tlvs
					.iter()
					.find(|tlv| tlv.tag() == nonce)
					.map(Tlv::parse::<Explicit<'a, _, 1>>)
					.transpose()?
					.map(Explicit::into_inner),
			})
		})
	}
}

pub struct DeviceAttestation<'a> {
	pub key_usage_properties: DeviceAttestationKeyUsageProperties<'a>,
	pub device_os_information: DeviceAttestationDeviceOSInformation<'a>,
	pub nonce: DeviceAttestationNonce<'a>,
}

#[allow(clippy::large_enum_variant)]
pub enum ParsedAttestation<'a> {
	KeyDescription(KeyDescription<'a>),
	DeviceAttestation(DeviceAttestation<'a>),
}

/// One of
/// Software (0),
/// TrustedEnvironment (1),
/// StrongBox (2) -> only exists in attestation version >= 3
pub type SecurityLevel = Enumerated;

/// Selective ASN.1 decoder for the Android KeyDescription `AuthorizationList`
/// SEQUENCE. Tag-whitelist parsing (via [`try_parse_tags`]) skips fields not in
/// this struct, so additions in future schema versions do not break decoding.
/// The whitelist is the union of fields used downstream (see
/// `BoundedAuthorizationList`) across V1..V4 and KeyMint (V100/V200/V300/V400).
pub struct AuthorizationList<'a> {
	pub purpose: Option<UnorderedSetOf<i64>>,
	pub algorithm: Option<i64>,
	pub key_size: Option<i64>,
	pub digest: Option<SetOf<'a, i64>>,
	pub padding: Option<SetOf<'a, i64>>,
	pub ec_curve: Option<i64>,
	pub rsa_public_exponent: Option<i64>,
	pub mgf_digest: Option<SetOf<'a, i64>>,
	pub rollback_resistance: Option<Null>,
	pub early_boot_only: Option<Null>,
	pub active_date_time: Option<i64>,
	pub origination_expire_date_time: Option<i64>,
	pub usage_expire_date_time: Option<i64>,
	pub usage_count_limit: Option<i64>,
	pub no_auth_required: Option<Null>,
	pub user_auth_type: Option<i64>,
	pub auth_timeout: Option<i64>,
	pub allow_while_on_body: Option<Null>,
	pub trusted_user_presence_required: Option<Null>,
	pub trusted_confirmation_required: Option<Null>,
	pub unlocked_device_required: Option<Null>,
	pub all_applications: Option<Null>,
	pub application_id: Option<&'a [u8]>,
	pub creation_date_time: Option<i64>,
	pub origin: Option<i64>,
	pub root_of_trust: Option<RootOfTrust<'a>>,
	pub os_version: Option<i64>,
	pub os_patch_level: Option<i64>,
	pub attestation_application_id: Option<&'a [u8]>,
	pub attestation_id_brand: Option<&'a [u8]>,
	pub attestation_id_device: Option<&'a [u8]>,
	pub attestation_id_product: Option<&'a [u8]>,
	pub attestation_id_serial: Option<&'a [u8]>,
	pub attestation_id_imei: Option<&'a [u8]>,
	pub attestation_id_meid: Option<&'a [u8]>,
	pub attestation_id_manufacturer: Option<&'a [u8]>,
	pub attestation_id_model: Option<&'a [u8]>,
	pub vendor_patch_level: Option<i64>,
	pub boot_patch_level: Option<i64>,
	pub device_unique_attestation: Option<Null>,
}

impl<'a> SimpleAsn1Readable<'a> for AuthorizationList<'a> {
	const TAG: Tag = <asn1::Sequence as SimpleAsn1Readable>::TAG;

	fn parse_data(data: &'a [u8]) -> ParseResult<Self> {
		// DER tag bytes for each `[explicit N]` context-specific constructed
		// wrapper (see Android Keystore tag list).
		// https://source.android.com/docs/security/keystore/tags
		asn1::parse(data, |parser| {
			let purpose_tag = Tag::from_bytes(&[0xA1, 0x00])?.0;
			let algorithm_tag = Tag::from_bytes(&[0xA2, 0x00])?.0;
			let key_size_tag = Tag::from_bytes(&[0xA3, 0x00])?.0;
			let digest_tag = Tag::from_bytes(&[0xA5, 0x00])?.0;
			let padding_tag = Tag::from_bytes(&[0xA6, 0x00])?.0;
			let ec_curve_tag = Tag::from_bytes(&[0xAA, 0x00])?.0;
			let rsa_public_exponent_tag = Tag::from_bytes(&[0xBF, 0x81, 0x48, 0x00])?.0;
			let mgf_digest_tag = Tag::from_bytes(&[0xBF, 0x81, 0x4B, 0x00])?.0;
			let rollback_resistance_tag = Tag::from_bytes(&[0xBF, 0x82, 0x2F, 0x00])?.0;
			let early_boot_only_tag = Tag::from_bytes(&[0xBF, 0x82, 0x31, 0x00])?.0;
			let active_date_time_tag = Tag::from_bytes(&[0xBF, 0x83, 0x10, 0x00])?.0;
			let origination_expire_date_time_tag = Tag::from_bytes(&[0xBF, 0x83, 0x11, 0x00])?.0;
			let usage_expire_date_time_tag = Tag::from_bytes(&[0xBF, 0x83, 0x12, 0x00])?.0;
			let usage_count_limit_tag = Tag::from_bytes(&[0xBF, 0x83, 0x15, 0x00])?.0;
			let no_auth_required_tag = Tag::from_bytes(&[0xBF, 0x83, 0x77, 0x00])?.0;
			let user_auth_type_tag = Tag::from_bytes(&[0xBF, 0x83, 0x78, 0x00])?.0;
			let auth_timeout_tag = Tag::from_bytes(&[0xBF, 0x83, 0x79, 0x00])?.0;
			let allow_while_on_body_tag = Tag::from_bytes(&[0xBF, 0x83, 0x7A, 0x00])?.0;
			let trusted_user_presence_required_tag = Tag::from_bytes(&[0xBF, 0x83, 0x7B, 0x00])?.0;
			let trusted_confirmation_required_tag = Tag::from_bytes(&[0xBF, 0x83, 0x7C, 0x00])?.0;
			let unlocked_device_required_tag = Tag::from_bytes(&[0xBF, 0x83, 0x7D, 0x00])?.0;
			let all_applications_tag = Tag::from_bytes(&[0xBF, 0x84, 0x58, 0x00])?.0;
			let application_id_tag = Tag::from_bytes(&[0xBF, 0x84, 0x59, 0x00])?.0;
			let creation_date_time_tag = Tag::from_bytes(&[0xBF, 0x85, 0x3D, 0x00])?.0;
			let origin_tag = Tag::from_bytes(&[0xBF, 0x85, 0x3E, 0x00])?.0;
			let root_of_trust_tag = Tag::from_bytes(&[0xBF, 0x85, 0x40, 0x00])?.0;
			let os_version_tag = Tag::from_bytes(&[0xBF, 0x85, 0x41, 0x00])?.0;
			let os_patch_level_tag = Tag::from_bytes(&[0xBF, 0x85, 0x42, 0x00])?.0;
			let attestation_application_id_tag = Tag::from_bytes(&[0xBF, 0x85, 0x45, 0x00])?.0;
			let attestation_id_brand_tag = Tag::from_bytes(&[0xBF, 0x85, 0x46, 0x00])?.0;
			let attestation_id_device_tag = Tag::from_bytes(&[0xBF, 0x85, 0x47, 0x00])?.0;
			let attestation_id_product_tag = Tag::from_bytes(&[0xBF, 0x85, 0x48, 0x00])?.0;
			let attestation_id_serial_tag = Tag::from_bytes(&[0xBF, 0x85, 0x49, 0x00])?.0;
			let attestation_id_imei_tag = Tag::from_bytes(&[0xBF, 0x85, 0x4A, 0x00])?.0;
			let attestation_id_meid_tag = Tag::from_bytes(&[0xBF, 0x85, 0x4B, 0x00])?.0;
			let attestation_id_manufacturer_tag = Tag::from_bytes(&[0xBF, 0x85, 0x4C, 0x00])?.0;
			let attestation_id_model_tag = Tag::from_bytes(&[0xBF, 0x85, 0x4D, 0x00])?.0;
			let vendor_patch_level_tag = Tag::from_bytes(&[0xBF, 0x85, 0x4E, 0x00])?.0;
			let boot_patch_level_tag = Tag::from_bytes(&[0xBF, 0x85, 0x4F, 0x00])?.0;
			let device_unique_attestation_tag = Tag::from_bytes(&[0xBF, 0x85, 0x50, 0x00])?.0;
			let tlvs = try_parse_tags(
				parser,
				&[
					purpose_tag,
					algorithm_tag,
					key_size_tag,
					digest_tag,
					padding_tag,
					ec_curve_tag,
					rsa_public_exponent_tag,
					mgf_digest_tag,
					rollback_resistance_tag,
					early_boot_only_tag,
					active_date_time_tag,
					origination_expire_date_time_tag,
					usage_expire_date_time_tag,
					usage_count_limit_tag,
					no_auth_required_tag,
					user_auth_type_tag,
					auth_timeout_tag,
					allow_while_on_body_tag,
					trusted_user_presence_required_tag,
					trusted_confirmation_required_tag,
					unlocked_device_required_tag,
					all_applications_tag,
					application_id_tag,
					creation_date_time_tag,
					origin_tag,
					root_of_trust_tag,
					os_version_tag,
					os_patch_level_tag,
					attestation_application_id_tag,
					attestation_id_brand_tag,
					attestation_id_device_tag,
					attestation_id_product_tag,
					attestation_id_serial_tag,
					attestation_id_imei_tag,
					attestation_id_meid_tag,
					attestation_id_manufacturer_tag,
					attestation_id_model_tag,
					vendor_patch_level_tag,
					boot_patch_level_tag,
					device_unique_attestation_tag,
				],
			)?;
			let pick = |tag: Tag| tlvs.iter().find(|tlv| tlv.tag() == tag);
			Ok(Self {
				purpose: pick(purpose_tag)
					.map(Tlv::parse::<Explicit<'a, _, 1>>)
					.transpose()?
					.map(Explicit::into_inner),
				algorithm: pick(algorithm_tag)
					.map(Tlv::parse::<Explicit<'a, _, 2>>)
					.transpose()?
					.map(Explicit::into_inner),
				key_size: pick(key_size_tag)
					.map(Tlv::parse::<Explicit<'a, _, 3>>)
					.transpose()?
					.map(Explicit::into_inner),
				digest: pick(digest_tag)
					.map(Tlv::parse::<Explicit<'a, _, 5>>)
					.transpose()?
					.map(Explicit::into_inner),
				padding: pick(padding_tag)
					.map(Tlv::parse::<Explicit<'a, _, 6>>)
					.transpose()?
					.map(Explicit::into_inner),
				ec_curve: pick(ec_curve_tag)
					.map(Tlv::parse::<Explicit<'a, _, 10>>)
					.transpose()?
					.map(Explicit::into_inner),
				rsa_public_exponent: pick(rsa_public_exponent_tag)
					.map(Tlv::parse::<Explicit<'a, _, 200>>)
					.transpose()?
					.map(Explicit::into_inner),
				mgf_digest: pick(mgf_digest_tag)
					.map(Tlv::parse::<Explicit<'a, _, 203>>)
					.transpose()?
					.map(Explicit::into_inner),
				rollback_resistance: pick(rollback_resistance_tag)
					.map(Tlv::parse::<Explicit<'a, _, 303>>)
					.transpose()?
					.map(Explicit::into_inner),
				early_boot_only: pick(early_boot_only_tag)
					.map(Tlv::parse::<Explicit<'a, _, 305>>)
					.transpose()?
					.map(Explicit::into_inner),
				active_date_time: pick(active_date_time_tag)
					.map(Tlv::parse::<Explicit<'a, _, 400>>)
					.transpose()?
					.map(Explicit::into_inner),
				origination_expire_date_time: pick(origination_expire_date_time_tag)
					.map(Tlv::parse::<Explicit<'a, _, 401>>)
					.transpose()?
					.map(Explicit::into_inner),
				usage_expire_date_time: pick(usage_expire_date_time_tag)
					.map(Tlv::parse::<Explicit<'a, _, 402>>)
					.transpose()?
					.map(Explicit::into_inner),
				usage_count_limit: pick(usage_count_limit_tag)
					.map(Tlv::parse::<Explicit<'a, _, 405>>)
					.transpose()?
					.map(Explicit::into_inner),
				no_auth_required: pick(no_auth_required_tag)
					.map(Tlv::parse::<Explicit<'a, _, 503>>)
					.transpose()?
					.map(Explicit::into_inner),
				user_auth_type: pick(user_auth_type_tag)
					.map(Tlv::parse::<Explicit<'a, _, 504>>)
					.transpose()?
					.map(Explicit::into_inner),
				auth_timeout: pick(auth_timeout_tag)
					.map(Tlv::parse::<Explicit<'a, _, 505>>)
					.transpose()?
					.map(Explicit::into_inner),
				allow_while_on_body: pick(allow_while_on_body_tag)
					.map(Tlv::parse::<Explicit<'a, _, 506>>)
					.transpose()?
					.map(Explicit::into_inner),
				trusted_user_presence_required: pick(trusted_user_presence_required_tag)
					.map(Tlv::parse::<Explicit<'a, _, 507>>)
					.transpose()?
					.map(Explicit::into_inner),
				trusted_confirmation_required: pick(trusted_confirmation_required_tag)
					.map(Tlv::parse::<Explicit<'a, _, 508>>)
					.transpose()?
					.map(Explicit::into_inner),
				unlocked_device_required: pick(unlocked_device_required_tag)
					.map(Tlv::parse::<Explicit<'a, _, 509>>)
					.transpose()?
					.map(Explicit::into_inner),
				all_applications: pick(all_applications_tag)
					.map(Tlv::parse::<Explicit<'a, _, 600>>)
					.transpose()?
					.map(Explicit::into_inner),
				application_id: pick(application_id_tag)
					.map(Tlv::parse::<Explicit<'a, _, 601>>)
					.transpose()?
					.map(Explicit::into_inner),
				creation_date_time: pick(creation_date_time_tag)
					.map(Tlv::parse::<Explicit<'a, _, 701>>)
					.transpose()?
					.map(Explicit::into_inner),
				origin: pick(origin_tag)
					.map(Tlv::parse::<Explicit<'a, _, 702>>)
					.transpose()?
					.map(Explicit::into_inner),
				root_of_trust: pick(root_of_trust_tag)
					.map(Tlv::parse::<Explicit<'a, _, 704>>)
					.transpose()?
					.map(Explicit::into_inner),
				os_version: pick(os_version_tag)
					.map(Tlv::parse::<Explicit<'a, _, 705>>)
					.transpose()?
					.map(Explicit::into_inner),
				os_patch_level: pick(os_patch_level_tag)
					.map(Tlv::parse::<Explicit<'a, _, 706>>)
					.transpose()?
					.map(Explicit::into_inner),
				attestation_application_id: pick(attestation_application_id_tag)
					.map(Tlv::parse::<Explicit<'a, _, 709>>)
					.transpose()?
					.map(Explicit::into_inner),
				attestation_id_brand: pick(attestation_id_brand_tag)
					.map(Tlv::parse::<Explicit<'a, _, 710>>)
					.transpose()?
					.map(Explicit::into_inner),
				attestation_id_device: pick(attestation_id_device_tag)
					.map(Tlv::parse::<Explicit<'a, _, 711>>)
					.transpose()?
					.map(Explicit::into_inner),
				attestation_id_product: pick(attestation_id_product_tag)
					.map(Tlv::parse::<Explicit<'a, _, 712>>)
					.transpose()?
					.map(Explicit::into_inner),
				attestation_id_serial: pick(attestation_id_serial_tag)
					.map(Tlv::parse::<Explicit<'a, _, 713>>)
					.transpose()?
					.map(Explicit::into_inner),
				attestation_id_imei: pick(attestation_id_imei_tag)
					.map(Tlv::parse::<Explicit<'a, _, 714>>)
					.transpose()?
					.map(Explicit::into_inner),
				attestation_id_meid: pick(attestation_id_meid_tag)
					.map(Tlv::parse::<Explicit<'a, _, 715>>)
					.transpose()?
					.map(Explicit::into_inner),
				attestation_id_manufacturer: pick(attestation_id_manufacturer_tag)
					.map(Tlv::parse::<Explicit<'a, _, 716>>)
					.transpose()?
					.map(Explicit::into_inner),
				attestation_id_model: pick(attestation_id_model_tag)
					.map(Tlv::parse::<Explicit<'a, _, 717>>)
					.transpose()?
					.map(Explicit::into_inner),
				vendor_patch_level: pick(vendor_patch_level_tag)
					.map(Tlv::parse::<Explicit<'a, _, 718>>)
					.transpose()?
					.map(Explicit::into_inner),
				boot_patch_level: pick(boot_patch_level_tag)
					.map(Tlv::parse::<Explicit<'a, _, 719>>)
					.transpose()?
					.map(Explicit::into_inner),
				device_unique_attestation: pick(device_unique_attestation_tag)
					.map(Tlv::parse::<Explicit<'a, _, 720>>)
					.transpose()?
					.map(Explicit::into_inner),
			})
		})
	}
}

#[derive(asn1::Asn1Read, asn1::Asn1Write)]
pub struct RootOfTrust<'a> {
	pub verified_boot_key: &'a [u8],
	pub device_locked: bool,
	pub verified_boot_state: VerifiedBootState,
	pub verified_boot_hash: Option<&'a [u8]>,
}

#[derive(asn1::Asn1Read, asn1::Asn1Write)]
pub struct RSAPublicKey<'a> {
	pub modulus: asn1::BigUint<'a>,
	pub exponent: asn1::BigUint<'a>,
}

#[derive(asn1::Asn1Read, asn1::Asn1Write)]
pub struct ECDSASignature<'a> {
	pub r: asn1::BigInt<'a>,
	pub s: asn1::BigInt<'a>,
}

#[derive(asn1::Asn1Read, asn1::Asn1Write)]
pub struct AttestationApplicationId<'a> {
	pub package_infos: SetOf<'a, AttestationPackageInfo<'a>>,
	pub signature_digests: SetOf<'a, &'a [u8]>,
}

#[derive(asn1::Asn1Read, asn1::Asn1Write)]
pub struct AttestationPackageInfo<'a> {
	pub package_name: &'a [u8],
	pub version: i64,
}

/// One of Verified (0),
/// SelfSigned (1),
/// Unverified (2),
/// Failed (3)
pub type VerifiedBootState = Enumerated;

/// Represents an ASN.1 `SET OF`. This is an `Iterator` over values that
/// are decoded.
pub struct UnorderedSetOf<T> {
	elements: Vec<T>,
}

impl<T> UnorderedSetOf<T> {
	fn new(elements: Vec<T>) -> Self {
		Self { elements }
	}

	pub fn elements(&self) -> &[T] {
		&self.elements
	}

	pub fn to_vec(self) -> Vec<T> {
		self.elements
	}
}

impl<'a, T: Asn1Readable<'a> + Clone> Clone for UnorderedSetOf<T> {
	fn clone(&self) -> UnorderedSetOf<T> {
		UnorderedSetOf { elements: self.elements.clone() }
	}
}

impl<'a, T: Asn1Readable<'a> + PartialEq> PartialEq for UnorderedSetOf<T> {
	fn eq(&self, other: &Self) -> bool {
		self.elements().eq(other.elements())
	}
}

impl<'a, T: Asn1Readable<'a> + Hash + Clone> Hash for UnorderedSetOf<T> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		for val in self.elements() {
			val.hash(state);
		}
	}
}

impl<'a, T: Asn1Readable<'a> + 'a> SimpleAsn1Readable<'a> for UnorderedSetOf<T> {
	const TAG: Tag = <SetOf<T> as SimpleAsn1Readable<'a>>::TAG;

	#[inline]
	fn parse_data(data: &'a [u8]) -> ParseResult<Self> {
		parse(data, |parser| {
			let mut elements = Vec::<T>::new();
			while !parser.is_empty() {
				let el = parser.read_element::<T>()?;
				elements.push(el)
			}
			Ok(Self::new(elements))
		})
	}
}

impl<'a, T: Asn1Readable<'a> + Asn1Writable + Clone> SimpleAsn1Writable for UnorderedSetOf<T> {
	const TAG: Tag = <SetOf<T> as SimpleAsn1Writable>::TAG;
	fn write_data(&self, _dest: &mut WriteBuf) -> WriteResult {
		unimplemented!();
	}
}
