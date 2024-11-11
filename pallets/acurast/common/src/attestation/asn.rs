#![cfg_attr(all(not(feature = "std"), not(test)), no_std)]

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

#[derive(Asn1Read, Asn1Write, Clone)]
pub struct Extension<'a> {
	pub extn_id: ObjectIdentifier,
	#[default(false)]
	pub critical: bool,
	/// contains the DER encoding of an ASN.1 value
	/// corresponding to the extension type identified by extnID
	pub extn_value: &'a [u8],
}

#[derive(Asn1Read, Asn1Write)]
pub struct KeyDescriptionV1<'a> {
	/// The [version](https://developer.android.com/training/articles/security-key-attestation#certificate_schema) of the attestation.
	/// It's necessary to peak this field before parsing all fields, since fields differ in versions and ASN parsing fails with a single deviating field.
	pub attestation_version: i64,
	pub attestation_security_level: SecurityLevel,
	pub key_mint_version: i64,
	pub key_mint_security_level: SecurityLevel,
	pub attestation_challenge: &'a [u8],
	pub unique_id: &'a [u8],
	pub software_enforced: AuthorizationListV1<'a>,
	pub tee_enforced: AuthorizationListV1<'a>,
}

#[derive(Asn1Read, Asn1Write)]
pub struct KeyDescriptionV2<'a> {
	/// The [version](https://developer.android.com/training/articles/security-key-attestation#certificate_schema) of the attestation.
	/// It's necessary to peak this field before parsing all fields, since fields differ in versions and ASN parsing fails with a single deviating field.
	pub attestation_version: i64,
	pub attestation_security_level: SecurityLevel,
	pub key_mint_version: i64,
	pub key_mint_security_level: SecurityLevel,
	pub attestation_challenge: &'a [u8],
	pub unique_id: &'a [u8],
	pub software_enforced: AuthorizationListV2<'a>,
	pub tee_enforced: AuthorizationListV2<'a>,
}

#[derive(Asn1Read, Asn1Write)]
pub struct KeyDescriptionV3<'a> {
	/// The [version](https://developer.android.com/training/articles/security-key-attestation#certificate_schema) of the attestation.
	/// It's necessary to peak this field before parsing all fields, since fields differ in versions and ASN parsing fails with a single deviating field.
	pub attestation_version: i64,
	pub attestation_security_level: SecurityLevel,
	pub key_mint_version: i64,
	pub key_mint_security_level: SecurityLevel,
	pub attestation_challenge: &'a [u8],
	pub unique_id: &'a [u8],
	pub software_enforced: AuthorizationListV3<'a>,
	pub tee_enforced: AuthorizationListV3<'a>,
}
#[derive(Asn1Read, Asn1Write)]
pub struct KeyDescriptionV4<'a> {
	/// The [version](https://developer.android.com/training/articles/security-key-attestation#certificate_schema) of the attestation.
	/// It's necessary to peak this field before parsing all fields, since fields differ in versions and ASN parsing fails with a single deviating field.
	pub attestation_version: i64,
	pub attestation_security_level: SecurityLevel,
	pub key_mint_version: i64,
	pub key_mint_security_level: SecurityLevel,
	pub attestation_challenge: &'a [u8],
	pub unique_id: &'a [u8],
	pub software_enforced: AuthorizationListV4<'a>,
	pub tee_enforced: AuthorizationListV4<'a>,
}

#[derive(Asn1Read, Asn1Write)]
pub struct KeyDescriptionKeyMint<'a> {
	/// The [version](https://developer.android.com/training/articles/security-key-attestation#certificate_schema) of the attestation.
	/// It's necessary to peak this field before parsing all fields, since fields differ in versions and ASN parsing fails with a single deviating field.
	pub attestation_version: i64,
	pub attestation_security_level: SecurityLevel,
	pub key_mint_version: i64,
	pub key_mint_security_level: SecurityLevel,
	pub attestation_challenge: &'a [u8],
	pub unique_id: &'a [u8],
	pub software_enforced: AuthorizationListKeyMint<'a>,
	pub tee_enforced: AuthorizationListKeyMint<'a>,
}

pub enum KeyDescription<'a> {
	V1(KeyDescriptionV1<'a>),
	V2(KeyDescriptionV2<'a>),
	V3(KeyDescriptionV3<'a>),
	V4(KeyDescriptionV4<'a>),
	V100(KeyDescriptionKeyMint<'a>),
	V200(KeyDescriptionKeyMint<'a>),
	V300(KeyDescriptionKeyMint<'a>),
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

pub enum ParsedAttestation<'a> {
	KeyDescription(KeyDescription<'a>),
	DeviceAttestation(DeviceAttestation<'a>),
}

/// One of
/// Software (0),
/// TrustedEnvironment (1),
/// StrongBox (2) -> only exists in attestation version >= 3
pub type SecurityLevel = Enumerated;

#[derive(asn1::Asn1Read, asn1::Asn1Write)]
pub struct AuthorizationListV1<'a> {
	#[explicit(1)]
	pub purpose: Option<UnorderedSetOf<i64>>,
	#[explicit(2)]
	pub algorithm: Option<i64>,
	#[explicit(3)]
	pub key_size: Option<i64>,
	#[explicit(5)]
	pub digest: Option<SetOf<'a, i64>>,
	#[explicit(6)]
	pub padding: Option<SetOf<'a, i64>>,
	#[explicit(10)]
	pub ec_curve: Option<i64>,
	#[explicit(200)]
	pub rsa_public_exponent: Option<i64>,
	#[explicit(303)]
	pub rollback_resistance: Option<Null>,
	#[explicit(400)]
	pub active_date_time: Option<i64>,
	#[explicit(401)]
	pub origination_expire_date_time: Option<i64>,
	#[explicit(402)]
	pub usage_expire_date_time: Option<i64>,
	#[explicit(503)]
	pub no_auth_required: Option<Null>,
	#[explicit(504)]
	pub user_auth_type: Option<i64>,
	#[explicit(505)]
	pub auth_timeout: Option<i64>,
	#[explicit(506)]
	pub allow_while_on_body: Option<Null>,
	#[explicit(507)]
	pub trusted_user_presence_required: Option<Null>,
	#[explicit(508)]
	pub trusted_confirmation_required: Option<Null>,
	#[explicit(509)]
	pub unlocked_device_required: Option<Null>,
	#[explicit(600)]
	pub all_applications: Option<Null>,
	#[explicit(601)]
	pub application_id: Option<&'a [u8]>,
	#[explicit(701)]
	pub creation_date_time: Option<i64>,
	#[explicit(702)]
	pub origin: Option<i64>,
	#[explicit(704)]
	pub root_of_trust: Option<RootOfTrustV1V2<'a>>,
	#[explicit(705)]
	pub os_version: Option<i64>,
	#[explicit(706)]
	pub os_patch_level: Option<i64>,
}

#[derive(asn1::Asn1Read, asn1::Asn1Write)]
pub struct AuthorizationListV2<'a> {
	#[explicit(1)]
	pub purpose: Option<UnorderedSetOf<i64>>,
	#[explicit(2)]
	pub algorithm: Option<i64>,
	#[explicit(3)]
	pub key_size: Option<i64>,
	#[explicit(5)]
	pub digest: Option<SetOf<'a, i64>>,
	#[explicit(6)]
	pub padding: Option<SetOf<'a, i64>>,
	#[explicit(10)]
	pub ec_curve: Option<i64>,
	#[explicit(200)]
	pub rsa_public_exponent: Option<i64>,
	#[explicit(303)]
	pub rollback_resistance: Option<Null>,
	#[explicit(400)]
	pub active_date_time: Option<i64>,
	#[explicit(401)]
	pub origination_expire_date_time: Option<i64>,
	#[explicit(402)]
	pub usage_expire_date_time: Option<i64>,
	#[explicit(503)]
	pub no_auth_required: Option<Null>,
	#[explicit(504)]
	pub user_auth_type: Option<i64>,
	#[explicit(505)]
	pub auth_timeout: Option<i64>,
	#[explicit(506)]
	pub allow_while_on_body: Option<Null>,
	#[explicit(507)]
	pub trusted_user_presence_required: Option<Null>,
	#[explicit(508)]
	pub trusted_confirmation_required: Option<Null>,
	#[explicit(509)]
	pub unlocked_device_required: Option<Null>,
	#[explicit(600)]
	pub all_applications: Option<Null>,
	#[explicit(601)]
	pub application_id: Option<&'a [u8]>,
	#[explicit(701)]
	pub creation_date_time: Option<i64>,
	#[explicit(702)]
	pub origin: Option<i64>,
	#[explicit(704)]
	pub root_of_trust: Option<RootOfTrustV1V2<'a>>,
	#[explicit(705)]
	pub os_version: Option<i64>,
	#[explicit(706)]
	pub os_patch_level: Option<i64>,
	#[explicit(709)]
	pub attestation_application_id: Option<&'a [u8]>,
	#[explicit(710)]
	pub attestation_id_brand: Option<&'a [u8]>,
	#[explicit(711)]
	pub attestation_id_device: Option<&'a [u8]>,
	#[explicit(712)]
	pub attestation_id_product: Option<&'a [u8]>,
	#[explicit(713)]
	pub attestation_id_serial: Option<&'a [u8]>,
	#[explicit(714)]
	pub attestation_id_imei: Option<&'a [u8]>,
	#[explicit(715)]
	pub attestation_id_meid: Option<&'a [u8]>,
	#[explicit(716)]
	pub attestation_id_manufacturer: Option<&'a [u8]>,
	#[explicit(717)]
	pub attestation_id_model: Option<&'a [u8]>,
}

#[derive(asn1::Asn1Read, asn1::Asn1Write)]
pub struct AuthorizationListV3<'a> {
	#[explicit(1)]
	pub purpose: Option<UnorderedSetOf<i64>>,
	#[explicit(2)]
	pub algorithm: Option<i64>,
	#[explicit(3)]
	pub key_size: Option<i64>,
	#[explicit(5)]
	pub digest: Option<SetOf<'a, i64>>,
	#[explicit(6)]
	pub padding: Option<SetOf<'a, i64>>,
	#[explicit(10)]
	pub ec_curve: Option<i64>,
	#[explicit(200)]
	pub rsa_public_exponent: Option<i64>,
	#[explicit(303)]
	pub rollback_resistance: Option<Null>,
	#[explicit(400)]
	pub active_date_time: Option<i64>,
	#[explicit(401)]
	pub origination_expire_date_time: Option<i64>,
	#[explicit(402)]
	pub usage_expire_date_time: Option<i64>,
	#[explicit(503)]
	pub no_auth_required: Option<Null>,
	#[explicit(504)]
	pub user_auth_type: Option<i64>,
	#[explicit(505)]
	pub auth_timeout: Option<i64>,
	#[explicit(506)]
	pub allow_while_on_body: Option<Null>,
	#[explicit(600)]
	pub all_applications: Option<Null>,
	#[explicit(601)]
	pub application_id: Option<&'a [u8]>,
	#[explicit(701)]
	pub creation_date_time: Option<i64>,
	#[explicit(702)]
	pub origin: Option<i64>,
	#[explicit(704)]
	pub root_of_trust: Option<RootOfTrust<'a>>,
	#[explicit(705)]
	pub os_version: Option<i64>,
	#[explicit(706)]
	pub os_patch_level: Option<i64>,
	#[explicit(709)]
	pub attestation_application_id: Option<&'a [u8]>,
	#[explicit(710)]
	pub attestation_id_brand: Option<&'a [u8]>,
	#[explicit(711)]
	pub attestation_id_device: Option<&'a [u8]>,
	#[explicit(712)]
	pub attestation_id_product: Option<&'a [u8]>,
	#[explicit(713)]
	pub attestation_id_serial: Option<&'a [u8]>,
	#[explicit(714)]
	pub attestation_id_imei: Option<&'a [u8]>,
	#[explicit(715)]
	pub attestation_id_meid: Option<&'a [u8]>,
	#[explicit(716)]
	pub attestation_id_manufacturer: Option<&'a [u8]>,
	#[explicit(717)]
	pub attestation_id_model: Option<&'a [u8]>,
	#[explicit(718)]
	pub vendor_patch_level: Option<i64>,
	#[explicit(719)]
	pub boot_patch_level: Option<i64>,
}

#[derive(asn1::Asn1Read, asn1::Asn1Write)]
pub struct RootOfTrustV1V2<'a> {
	pub verified_boot_key: &'a [u8],
	pub device_locked: bool,
	pub verified_boot_state: VerifiedBootState,
}

#[derive(asn1::Asn1Read, asn1::Asn1Write)]
pub struct AuthorizationListV4<'a> {
	#[explicit(1)]
	pub purpose: Option<UnorderedSetOf<i64>>,
	#[explicit(2)]
	pub algorithm: Option<i64>,
	#[explicit(3)]
	pub key_size: Option<i64>,
	#[explicit(5)]
	pub digest: Option<SetOf<'a, i64>>,
	#[explicit(6)]
	pub padding: Option<SetOf<'a, i64>>,
	#[explicit(10)]
	pub ec_curve: Option<i64>,
	#[explicit(200)]
	pub rsa_public_exponent: Option<i64>,
	#[explicit(303)]
	pub rollback_resistance: Option<Null>,
	#[explicit(305)]
	pub early_boot_only: Option<Null>,
	#[explicit(400)]
	pub active_date_time: Option<i64>,
	#[explicit(401)]
	pub origination_expire_date_time: Option<i64>,
	#[explicit(402)]
	pub usage_expire_date_time: Option<i64>,
	#[explicit(503)]
	pub no_auth_required: Option<Null>,
	#[explicit(504)]
	pub user_auth_type: Option<i64>,
	#[explicit(505)]
	pub auth_timeout: Option<i64>,
	#[explicit(506)]
	pub allow_while_on_body: Option<Null>,
	#[explicit(507)]
	pub trusted_user_presence_required: Option<Null>,
	#[explicit(508)]
	pub trusted_confirmation_required: Option<Null>,
	#[explicit(509)]
	pub unlocked_device_required: Option<Null>,
	#[explicit(600)]
	pub all_applications: Option<Null>,
	#[explicit(601)]
	pub application_id: Option<&'a [u8]>,
	#[explicit(701)]
	pub creation_date_time: Option<i64>,
	#[explicit(702)]
	pub origin: Option<i64>,
	#[explicit(704)]
	pub root_of_trust: Option<RootOfTrust<'a>>,
	#[explicit(705)]
	pub os_version: Option<i64>,
	#[explicit(706)]
	pub os_patch_level: Option<i64>,
	#[explicit(709)]
	pub attestation_application_id: Option<&'a [u8]>,
	#[explicit(710)]
	pub attestation_id_brand: Option<&'a [u8]>,
	#[explicit(711)]
	pub attestation_id_device: Option<&'a [u8]>,
	#[explicit(712)]
	pub attestation_id_product: Option<&'a [u8]>,
	#[explicit(713)]
	pub attestation_id_serial: Option<&'a [u8]>,
	#[explicit(714)]
	pub attestation_id_imei: Option<&'a [u8]>,
	#[explicit(715)]
	pub attestation_id_meid: Option<&'a [u8]>,
	#[explicit(716)]
	pub attestation_id_manufacturer: Option<&'a [u8]>,
	#[explicit(717)]
	pub attestation_id_model: Option<&'a [u8]>,
	#[explicit(718)]
	pub vendor_patch_level: Option<i64>,
	#[explicit(719)]
	pub boot_patch_level: Option<i64>,
	#[explicit(720)]
	pub device_unique_attestation: Option<Null>,
}

#[derive(asn1::Asn1Read, asn1::Asn1Write)]
pub struct AuthorizationListKeyMint<'a> {
	#[explicit(1)]
	pub purpose: Option<UnorderedSetOf<i64>>,
	#[explicit(2)]
	pub algorithm: Option<i64>,
	#[explicit(3)]
	pub key_size: Option<i64>,
	#[explicit(5)]
	pub digest: Option<SetOf<'a, i64>>,
	#[explicit(6)]
	pub padding: Option<SetOf<'a, i64>>,
	#[explicit(10)]
	pub ec_curve: Option<i64>,
	#[explicit(200)]
	pub rsa_public_exponent: Option<i64>,
	#[explicit(203)]
	pub mgf_digest: Option<SetOf<'a, i64>>,
	#[explicit(303)]
	pub rollback_resistance: Option<Null>,
	#[explicit(305)]
	pub early_boot_only: Option<Null>,
	#[explicit(400)]
	pub active_date_time: Option<i64>,
	#[explicit(401)]
	pub origination_expire_date_time: Option<i64>,
	#[explicit(402)]
	pub usage_expire_date_time: Option<i64>,
	#[explicit(405)]
	pub usage_count_limit: Option<i64>,
	#[explicit(503)]
	pub no_auth_required: Option<Null>,
	#[explicit(504)]
	pub user_auth_type: Option<i64>,
	#[explicit(505)]
	pub auth_timeout: Option<i64>,
	#[explicit(506)]
	pub allow_while_on_body: Option<Null>,
	#[explicit(507)]
	pub trusted_user_presence_required: Option<Null>,
	#[explicit(508)]
	pub trusted_confirmation_required: Option<Null>,
	#[explicit(509)]
	pub unlocked_device_required: Option<Null>,
	#[explicit(701)]
	pub creation_date_time: Option<i64>,
	#[explicit(702)]
	pub origin: Option<i64>,
	#[explicit(704)]
	pub root_of_trust: Option<RootOfTrust<'a>>,
	#[explicit(705)]
	pub os_version: Option<i64>,
	#[explicit(706)]
	pub os_patch_level: Option<i64>,
	#[explicit(709)]
	pub attestation_application_id: Option<&'a [u8]>,
	#[explicit(710)]
	pub attestation_id_brand: Option<&'a [u8]>,
	#[explicit(711)]
	pub attestation_id_device: Option<&'a [u8]>,
	#[explicit(712)]
	pub attestation_id_product: Option<&'a [u8]>,
	#[explicit(713)]
	pub attestation_id_serial: Option<&'a [u8]>,
	#[explicit(714)]
	pub attestation_id_imei: Option<&'a [u8]>,
	#[explicit(715)]
	pub attestation_id_meid: Option<&'a [u8]>,
	#[explicit(716)]
	pub attestation_id_manufacturer: Option<&'a [u8]>,
	#[explicit(717)]
	pub attestation_id_model: Option<&'a [u8]>,
	#[explicit(718)]
	pub vendor_patch_level: Option<i64>,
	#[explicit(719)]
	pub boot_patch_level: Option<i64>,
	#[explicit(720)]
	pub device_unique_attestation: Option<Null>,
}

#[derive(asn1::Asn1Read, asn1::Asn1Write)]
pub struct RootOfTrust<'a> {
	pub verified_boot_key: &'a [u8],
	pub device_locked: bool,
	pub verified_boot_state: VerifiedBootState,
	pub verified_boot_hash: &'a [u8],
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
