#![cfg(feature = "attestation")]

use crate::{
    attestation::{
        asn::{self, KeyDescription},
        CertificateChainInput, CHAIN_MAX_LENGTH,
    },
    SerialNumber,
};

use frame_support::{pallet_prelude::*, storage::bounded_vec::BoundedVec};
use serde::{Deserialize, Serialize};
use sp_std::prelude::*;

const ISSUER_NAME_MAX_LENGTH: u32 = 128;
pub type IssuerName = BoundedVec<u8, ConstU32<ISSUER_NAME_MAX_LENGTH>>;

pub(crate) const PURPOSE_MAX_LENGTH: u32 = 50;
pub(crate) const DIGEST_MAX_LENGTH: u32 = 32;
pub(crate) const PADDING_MAX_LENGTH: u32 = 32;
pub(crate) const MGF_DIGEST_MAX_LENGTH: u32 = 32;
pub(crate) const VERIFIED_BOOT_KEY_MAX_LENGTH: u32 = 32;
pub(crate) const VERIFIED_BOOT_HASH_MAX_LENGTH: u32 = 32;
pub(crate) const ATTESTATION_ID_MAX_LENGTH: u32 = 256;
pub(crate) const BOUNDED_SET_PROPERTY: u32 = 16;
pub(crate) const PACKAGE_NAME_MAX_LENGTH: u32 = 128;
pub(crate) const SIGNATURE_DIGEST_SET_MAX_LENGTH: u32 = 16;

pub type Purpose = BoundedVec<u8, ConstU32<PURPOSE_MAX_LENGTH>>;
pub type Digest = BoundedVec<u8, ConstU32<DIGEST_MAX_LENGTH>>;
pub type Padding = BoundedVec<u8, ConstU32<PADDING_MAX_LENGTH>>;
pub type MgfDigest = BoundedVec<u8, ConstU32<MGF_DIGEST_MAX_LENGTH>>;
pub type VerifiedBootKey = BoundedVec<u8, ConstU32<VERIFIED_BOOT_KEY_MAX_LENGTH>>;
pub type VerifiedBootHash = BoundedVec<u8, ConstU32<VERIFIED_BOOT_HASH_MAX_LENGTH>>;
pub type AttestationIdProperty = BoundedVec<u8, ConstU32<ATTESTATION_ID_MAX_LENGTH>>;
pub type CertId = (IssuerName, SerialNumber);
pub type ValidatingCertIds = BoundedVec<CertId, ConstU32<CHAIN_MAX_LENGTH>>;
pub type BoundedSetProperty = BoundedVec<CertId, ConstU32<BOUNDED_SET_PROPERTY>>;
pub type PackageName = BoundedVec<u8, ConstU32<PACKAGE_NAME_MAX_LENGTH>>;
pub type SignatureDigestSet = BoundedVec<Digest, ConstU32<SIGNATURE_DIGEST_SET_MAX_LENGTH>>;
pub type PackageInfoSet = BoundedVec<BoundedAttestationPackageInfo, ConstU32<16>>;

/// Structure representing a submitted attestation chain.
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq)]
pub struct AttestationChain {
    /// An ordered array of [CertificateInput]s describing a valid chain from known root certificate to attestation certificate.
    pub certificate_chain: CertificateChainInput,
}

/// Structure representing a stored attestation.
#[derive(
    RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Serialize, Deserialize,
)]
pub struct Attestation {
    pub cert_ids: ValidatingCertIds,
    pub key_description: BoundedKeyDescription,
    pub validity: AttestationValidity,
}

#[derive(
    RuntimeDebug,
    Encode,
    Decode,
    MaxEncodedLen,
    TypeInfo,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
)]
pub struct AttestationValidity {
    pub not_before: u64,
    pub not_after: u64,
}

#[derive(
    RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Serialize, Deserialize,
)]
pub struct BoundedKeyDescription {
    pub attestation_security_level: AttestationSecurityLevel,
    pub key_mint_security_level: AttestationSecurityLevel,
    pub software_enforced: BoundedAuthorizationList,
    pub tee_enforced: BoundedAuthorizationList,
}

impl TryFrom<KeyDescription<'_>> for BoundedKeyDescription {
    type Error = ();

    fn try_from(value: KeyDescription) -> Result<Self, Self::Error> {
        match value {
            KeyDescription::V1(kd) => kd.try_into(),
            KeyDescription::V2(kd) => kd.try_into(),
            KeyDescription::V3(kd) => kd.try_into(),
            KeyDescription::V4(kd) => kd.try_into(),
            KeyDescription::V100(kd) => kd.try_into(),
            KeyDescription::V200(kd) => kd.try_into(),
            KeyDescription::V300(kd) => kd.try_into(),
        }
    }
}

impl TryFrom<asn::KeyDescriptionV1<'_>> for BoundedKeyDescription {
    type Error = ();

    fn try_from(data: asn::KeyDescriptionV1) -> Result<Self, Self::Error> {
        Ok(BoundedKeyDescription {
            attestation_security_level: data.attestation_security_level.into(),
            key_mint_security_level: data.key_mint_security_level.into(),
            software_enforced: data.software_enforced.try_into()?,
            tee_enforced: data.tee_enforced.try_into()?,
        })
    }
}

impl TryFrom<asn::KeyDescriptionV2<'_>> for BoundedKeyDescription {
    type Error = ();

    fn try_from(data: asn::KeyDescriptionV2) -> Result<Self, Self::Error> {
        Ok(BoundedKeyDescription {
            attestation_security_level: data.attestation_security_level.into(),
            key_mint_security_level: data.key_mint_security_level.into(),
            software_enforced: data.software_enforced.try_into()?,
            tee_enforced: data.tee_enforced.try_into()?,
        })
    }
}

impl TryFrom<asn::KeyDescriptionV3<'_>> for BoundedKeyDescription {
    type Error = ();

    fn try_from(data: asn::KeyDescriptionV3) -> Result<Self, Self::Error> {
        Ok(BoundedKeyDescription {
            attestation_security_level: data.attestation_security_level.into(),
            key_mint_security_level: data.key_mint_security_level.into(),
            software_enforced: data.software_enforced.try_into()?,
            tee_enforced: data.tee_enforced.try_into()?,
        })
    }
}

impl TryFrom<asn::KeyDescriptionV4<'_>> for BoundedKeyDescription {
    type Error = ();

    fn try_from(data: asn::KeyDescriptionV4) -> Result<Self, Self::Error> {
        Ok(BoundedKeyDescription {
            attestation_security_level: data.attestation_security_level.into(),
            key_mint_security_level: data.key_mint_security_level.into(),
            software_enforced: data.software_enforced.try_into()?,
            tee_enforced: data.tee_enforced.try_into()?,
        })
    }
}

impl TryFrom<asn::KeyDescriptionKeyMint<'_>> for BoundedKeyDescription {
    type Error = ();

    fn try_from(data: asn::KeyDescriptionKeyMint) -> Result<Self, Self::Error> {
        Ok(BoundedKeyDescription {
            attestation_security_level: data.attestation_security_level.into(),
            key_mint_security_level: data.key_mint_security_level.into(),
            software_enforced: data.software_enforced.try_into()?,
            tee_enforced: data.tee_enforced.try_into()?,
        })
    }
}

#[derive(
    RuntimeDebug,
    Encode,
    Decode,
    MaxEncodedLen,
    TypeInfo,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
)]
pub enum AttestationSecurityLevel {
    Software,
    TrustedEnvironemnt,
    StrongBox,
    Unknown,
}

impl From<asn::SecurityLevel> for AttestationSecurityLevel {
    fn from(data: asn::SecurityLevel) -> Self {
        match data.value() {
            0 => AttestationSecurityLevel::Software,
            1 => AttestationSecurityLevel::TrustedEnvironemnt,
            2 => AttestationSecurityLevel::StrongBox,
            _ => AttestationSecurityLevel::Unknown,
        }
    }
}

#[derive(
    RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Serialize, Deserialize,
)]
pub struct BoundedAuthorizationList {
    pub purpose: Option<Purpose>,
    pub algorithm: Option<u8>,
    pub key_size: Option<u16>,
    pub digest: Option<Digest>,
    pub padding: Option<Padding>,
    pub ec_curve: Option<u8>,
    pub rsa_public_exponent: Option<u64>,
    pub mgf_digest: Option<MgfDigest>,
    pub rollback_resistance: Option<bool>,
    pub early_boot_only: Option<bool>,
    pub active_date_time: Option<u64>,
    pub origination_expire_date_time: Option<u64>,
    pub usage_expire_date_time: Option<u64>,
    pub usage_count_limit: Option<u64>,
    pub no_auth_required: bool,
    pub user_auth_type: Option<u8>,
    pub auth_timeout: Option<u32>,
    pub allow_while_on_body: bool,
    pub trusted_user_presence_required: Option<bool>,
    pub trusted_confirmation_required: Option<bool>,
    pub unlocked_device_required: Option<bool>,
    pub all_applications: Option<bool>,
    pub application_id: Option<AttestationIdProperty>,
    pub creation_date_time: Option<u64>,
    pub origin: Option<u8>,
    pub root_of_trust: Option<BoundedRootOfTrust>,
    pub os_version: Option<u32>,
    pub os_patch_level: Option<u32>,
    pub attestation_application_id: Option<BoundedAttestationApplicationId>,
    pub attestation_id_brand: Option<AttestationIdProperty>,
    pub attestation_id_device: Option<AttestationIdProperty>,
    pub attestation_id_product: Option<AttestationIdProperty>,
    pub attestation_id_serial: Option<AttestationIdProperty>,
    pub attestation_id_imei: Option<AttestationIdProperty>,
    pub attestation_id_meid: Option<AttestationIdProperty>,
    pub attestation_id_manufacturer: Option<AttestationIdProperty>,
    pub attestation_id_model: Option<AttestationIdProperty>,
    pub vendor_patch_level: Option<u32>,
    pub boot_patch_level: Option<u32>,
    pub device_unique_attestation: Option<bool>,
}

macro_rules! try_bound_set {
    ( $set:expr, $target_vec_type:ty, $target_type:ty ) => {{
        $set.map(|v| {
            v.map(|i| <$target_type>::try_from(i))
                .collect::<Result<Vec<$target_type>, _>>()
        })
        .map_or(Ok(None), |r| r.map(Some))
        .map_err(|_| ())?
        .map(|v| <$target_vec_type>::try_from(v))
        .map_or(Ok(None), |r| r.map(Some))
        .map_err(|_| ())
    }};
}

macro_rules! try_bound {
    ( $v:expr, $target_type:ty ) => {{
        $v.map(|v| <$target_type>::try_from(v))
            .map_or(Ok(None), |r| r.map(Some))
            .map_err(|_| ())
    }};
}

/// The Authorization List tags. [Tag descriptions](https://source.android.com/docs/security/keystore/tags)
impl TryFrom<asn::AuthorizationListV1<'_>> for BoundedAuthorizationList {
    type Error = ();

    fn try_from(data: asn::AuthorizationListV1) -> Result<Self, Self::Error> {
        Ok(BoundedAuthorizationList {
            purpose: try_bound_set!(data.purpose.map(|v| v.to_vec().into_iter()), Purpose, u8)?,
            algorithm: try_bound!(data.algorithm, u8)?,
            key_size: try_bound!(data.key_size, u16)?,
            digest: try_bound_set!(data.digest, Digest, u8)?,
            padding: try_bound_set!(data.padding, Padding, u8)?,
            ec_curve: try_bound!(data.ec_curve, u8)?,
            rsa_public_exponent: try_bound!(data.rsa_public_exponent, u64)?,
            mgf_digest: None,
            rollback_resistance: Some(data.rollback_resistance.is_some()),
            early_boot_only: None,
            active_date_time: try_bound!(data.active_date_time, u64)?,
            origination_expire_date_time: try_bound!(data.origination_expire_date_time, u64)?,
            usage_expire_date_time: try_bound!(data.usage_expire_date_time, u64)?,
            usage_count_limit: None,
            no_auth_required: data.no_auth_required.is_some(),
            user_auth_type: try_bound!(data.user_auth_type, u8)?,
            auth_timeout: try_bound!(data.user_auth_type, u32)?,
            allow_while_on_body: data.allow_while_on_body.is_some(),
            trusted_user_presence_required: None,
            trusted_confirmation_required: None,
            unlocked_device_required: None,
            all_applications: Some(data.all_applications.is_some()),
            application_id: data
                .application_id
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            creation_date_time: try_bound!(data.creation_date_time, u64)?,
            origin: try_bound!(data.origin, u8)?,
            root_of_trust: data
                .root_of_trust
                .map(|v| v.try_into())
                .map_or(Ok(None), |r| r.map(Some))?,
            os_version: try_bound!(data.os_version, u32)?,
            os_patch_level: try_bound!(data.os_patch_level, u32)?,
            vendor_patch_level: None,
            attestation_application_id: None,
            attestation_id_brand: None,
            attestation_id_device: None,
            attestation_id_product: None,
            attestation_id_serial: None,
            attestation_id_imei: None,
            attestation_id_meid: None,
            attestation_id_manufacturer: None,
            attestation_id_model: None,
            boot_patch_level: None,
            device_unique_attestation: None,
        })
    }
}

impl TryFrom<asn::AuthorizationListV2<'_>> for BoundedAuthorizationList {
    type Error = ();

    fn try_from(data: asn::AuthorizationListV2) -> Result<Self, Self::Error> {
        Ok(BoundedAuthorizationList {
            purpose: try_bound_set!(data.purpose.map(|v| v.to_vec().into_iter()), Purpose, u8)?,
            algorithm: try_bound!(data.algorithm, u8)?,
            key_size: try_bound!(data.key_size, u16)?,
            digest: try_bound_set!(data.digest, Digest, u8)?,
            padding: try_bound_set!(data.padding, Padding, u8)?,
            ec_curve: try_bound!(data.ec_curve, u8)?,
            rsa_public_exponent: try_bound!(data.rsa_public_exponent, u64)?,
            mgf_digest: None,
            rollback_resistance: Some(data.rollback_resistance.is_some()),
            early_boot_only: None,
            active_date_time: try_bound!(data.active_date_time, u64)?,
            origination_expire_date_time: try_bound!(data.origination_expire_date_time, u64)?,
            usage_expire_date_time: try_bound!(data.usage_expire_date_time, u64)?,
            usage_count_limit: None,
            no_auth_required: data.no_auth_required.is_some(),
            user_auth_type: try_bound!(data.user_auth_type, u8)?,
            auth_timeout: try_bound!(data.user_auth_type, u32)?,
            allow_while_on_body: data.allow_while_on_body.is_some(),
            trusted_user_presence_required: None,
            trusted_confirmation_required: None,
            unlocked_device_required: None,
            all_applications: Some(data.all_applications.is_some()),
            application_id: data
                .application_id
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            creation_date_time: try_bound!(data.creation_date_time, u64)?,
            origin: try_bound!(data.origin, u8)?,
            root_of_trust: data
                .root_of_trust
                .map(|v| v.try_into())
                .map_or(Ok(None), |r| r.map(Some))?,
            os_version: try_bound!(data.os_version, u32)?,
            os_patch_level: try_bound!(data.os_patch_level, u32)?,
            attestation_application_id: data
                .attestation_application_id
                .map(|bytes| {
                    asn1::parse_single::<asn::AttestationApplicationId>(bytes)
                        .map_err(|_| ())
                        .and_then(|app_id| BoundedAttestationApplicationId::try_from(app_id))
                })
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_brand: data
                .attestation_id_brand
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_device: data
                .attestation_id_device
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_product: data
                .attestation_id_product
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_serial: data
                .attestation_id_serial
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_imei: data
                .attestation_id_imei
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_meid: data
                .attestation_id_meid
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_manufacturer: data
                .attestation_id_manufacturer
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_model: data
                .attestation_id_model
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            vendor_patch_level: None,
            boot_patch_level: None,
            device_unique_attestation: None,
        })
    }
}

impl TryFrom<asn::AuthorizationListV3<'_>> for BoundedAuthorizationList {
    type Error = ();

    fn try_from(data: asn::AuthorizationListV3) -> Result<Self, Self::Error> {
        Ok(BoundedAuthorizationList {
            purpose: try_bound_set!(data.purpose.map(|v| v.to_vec().into_iter()), Purpose, u8)?,
            algorithm: try_bound!(data.algorithm, u8)?,
            key_size: try_bound!(data.key_size, u16)?,
            digest: try_bound_set!(data.digest, Digest, u8)?,
            padding: try_bound_set!(data.padding, Padding, u8)?,
            ec_curve: try_bound!(data.ec_curve, u8)?,
            rsa_public_exponent: try_bound!(data.rsa_public_exponent, u64)?,
            mgf_digest: None,
            rollback_resistance: Some(data.rollback_resistance.is_some()),
            early_boot_only: None,
            active_date_time: try_bound!(data.active_date_time, u64)?,
            origination_expire_date_time: try_bound!(data.origination_expire_date_time, u64)?,
            usage_expire_date_time: try_bound!(data.usage_expire_date_time, u64)?,
            usage_count_limit: None,
            no_auth_required: data.no_auth_required.is_some(),
            user_auth_type: try_bound!(data.user_auth_type, u8)?,
            auth_timeout: try_bound!(data.user_auth_type, u32)?,
            allow_while_on_body: data.allow_while_on_body.is_some(),
            trusted_user_presence_required: None,
            trusted_confirmation_required: None,
            unlocked_device_required: None,
            all_applications: Some(data.all_applications.is_some()),
            application_id: data
                .application_id
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            creation_date_time: try_bound!(data.creation_date_time, u64)?,
            origin: try_bound!(data.origin, u8)?,
            root_of_trust: data
                .root_of_trust
                .map(|v| v.try_into())
                .map_or(Ok(None), |r| r.map(Some))?,
            os_version: try_bound!(data.os_version, u32)?,
            os_patch_level: try_bound!(data.os_patch_level, u32)?,
            attestation_application_id: data
                .attestation_application_id
                .map(|bytes| {
                    asn1::parse_single::<asn::AttestationApplicationId>(bytes)
                        .map_err(|_| ())
                        .and_then(|app_id| BoundedAttestationApplicationId::try_from(app_id))
                })
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_brand: data
                .attestation_id_brand
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_device: data
                .attestation_id_device
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_product: data
                .attestation_id_product
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_serial: data
                .attestation_id_serial
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_imei: data
                .attestation_id_imei
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_meid: data
                .attestation_id_meid
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_manufacturer: data
                .attestation_id_manufacturer
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_model: data
                .attestation_id_model
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            vendor_patch_level: try_bound!(data.vendor_patch_level, u32)?,
            boot_patch_level: try_bound!(data.boot_patch_level, u32)?,
            device_unique_attestation: None,
        })
    }
}

impl TryFrom<asn::AuthorizationListV4<'_>> for BoundedAuthorizationList {
    type Error = ();

    fn try_from(data: asn::AuthorizationListV4) -> Result<Self, Self::Error> {
        Ok(BoundedAuthorizationList {
            purpose: try_bound_set!(data.purpose.map(|v| v.to_vec().into_iter()), Purpose, u8)?,
            algorithm: try_bound!(data.algorithm, u8)?,
            key_size: try_bound!(data.key_size, u16)?,
            digest: try_bound_set!(data.digest, Digest, u8)?,
            padding: try_bound_set!(data.padding, Padding, u8)?,
            ec_curve: try_bound!(data.ec_curve, u8)?,
            rsa_public_exponent: try_bound!(data.rsa_public_exponent, u64)?,
            mgf_digest: None,
            rollback_resistance: Some(data.rollback_resistance.is_some()),
            early_boot_only: Some(data.early_boot_only.is_some()),
            active_date_time: try_bound!(data.active_date_time, u64)?,
            origination_expire_date_time: try_bound!(data.origination_expire_date_time, u64)?,
            usage_expire_date_time: try_bound!(data.usage_expire_date_time, u64)?,
            usage_count_limit: None,
            no_auth_required: data.no_auth_required.is_some(),
            user_auth_type: try_bound!(data.user_auth_type, u8)?,
            auth_timeout: try_bound!(data.user_auth_type, u32)?,
            allow_while_on_body: data.allow_while_on_body.is_some(),
            trusted_user_presence_required: Some(data.trusted_user_presence_required.is_some()),
            trusted_confirmation_required: Some(data.trusted_confirmation_required.is_some()),
            unlocked_device_required: Some(data.unlocked_device_required.is_some()),
            all_applications: Some(data.all_applications.is_some()),
            application_id: data
                .application_id
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            creation_date_time: try_bound!(data.creation_date_time, u64)?,
            origin: try_bound!(data.origin, u8)?,
            root_of_trust: data
                .root_of_trust
                .map(|v| v.try_into())
                .map_or(Ok(None), |r| r.map(Some))?,
            os_version: try_bound!(data.os_version, u32)?,
            os_patch_level: try_bound!(data.os_patch_level, u32)?,
            attestation_application_id: data
                .attestation_application_id
                .map(|bytes| {
                    asn1::parse_single::<asn::AttestationApplicationId>(bytes)
                        .map_err(|_| ())
                        .and_then(|app_id| BoundedAttestationApplicationId::try_from(app_id))
                })
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_brand: data
                .attestation_id_brand
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_device: data
                .attestation_id_device
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_product: data
                .attestation_id_product
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_serial: data
                .attestation_id_serial
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_imei: data
                .attestation_id_imei
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_meid: data
                .attestation_id_meid
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_manufacturer: data
                .attestation_id_manufacturer
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_model: data
                .attestation_id_model
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            vendor_patch_level: try_bound!(data.vendor_patch_level, u32)?,
            boot_patch_level: try_bound!(data.boot_patch_level, u32)?,
            device_unique_attestation: Some(data.device_unique_attestation.is_some()),
        })
    }
}

impl TryFrom<asn::AuthorizationListKeyMint<'_>> for BoundedAuthorizationList {
    type Error = ();

    fn try_from(data: asn::AuthorizationListKeyMint) -> Result<Self, Self::Error> {
        Ok(BoundedAuthorizationList {
            purpose: try_bound_set!(data.purpose.map(|v| v.to_vec().into_iter()), Purpose, u8)?,
            algorithm: try_bound!(data.algorithm, u8)?,
            key_size: try_bound!(data.key_size, u16)?,
            digest: try_bound_set!(data.digest, Digest, u8)?,
            padding: try_bound_set!(data.padding, Padding, u8)?,
            ec_curve: try_bound!(data.ec_curve, u8)?,
            rsa_public_exponent: try_bound!(data.rsa_public_exponent, u64)?,
            mgf_digest: try_bound_set!(data.mgf_digest, MgfDigest, u8)?,
            rollback_resistance: Some(data.rollback_resistance.is_some()),
            early_boot_only: Some(data.early_boot_only.is_some()),
            active_date_time: try_bound!(data.active_date_time, u64)?,
            origination_expire_date_time: try_bound!(data.origination_expire_date_time, u64)?,
            usage_expire_date_time: try_bound!(data.usage_expire_date_time, u64)?,
            usage_count_limit: try_bound!(data.usage_count_limit, u64)?,
            no_auth_required: data.no_auth_required.is_some(),
            user_auth_type: try_bound!(data.user_auth_type, u8)?,
            auth_timeout: try_bound!(data.user_auth_type, u32)?,
            allow_while_on_body: data.allow_while_on_body.is_some(),
            trusted_user_presence_required: Some(data.trusted_user_presence_required.is_some()),
            trusted_confirmation_required: Some(data.trusted_confirmation_required.is_some()),
            unlocked_device_required: Some(data.unlocked_device_required.is_some()),
            all_applications: None,
            application_id: None,
            creation_date_time: try_bound!(data.creation_date_time, u64)?,
            origin: try_bound!(data.origin, u8)?,
            root_of_trust: data
                .root_of_trust
                .map(|v| v.try_into())
                .map_or(Ok(None), |r| r.map(Some))?,
            os_version: try_bound!(data.os_version, u32)?,
            os_patch_level: try_bound!(data.os_patch_level, u32)?,
            attestation_application_id: data
                .attestation_application_id
                .map(|bytes| {
                    asn1::parse_single::<asn::AttestationApplicationId>(bytes)
                        .map_err(|_| ())
                        .and_then(|app_id| BoundedAttestationApplicationId::try_from(app_id))
                })
                .map_or(Ok(None), |r| r.map(Some))?,
            attestation_id_brand: data
                .attestation_id_brand
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_device: data
                .attestation_id_device
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_product: data
                .attestation_id_product
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_serial: data
                .attestation_id_serial
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_imei: data
                .attestation_id_imei
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_meid: data
                .attestation_id_meid
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_manufacturer: data
                .attestation_id_manufacturer
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            attestation_id_model: data
                .attestation_id_model
                .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())?,
            vendor_patch_level: try_bound!(data.vendor_patch_level, u32)?,
            boot_patch_level: try_bound!(data.boot_patch_level, u32)?,
            device_unique_attestation: Some(data.device_unique_attestation.is_some()),
        })
    }
}

#[derive(
    RuntimeDebug,
    Encode,
    Decode,
    MaxEncodedLen,
    TypeInfo,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
)]
pub struct BoundedRootOfTrust {
    pub verified_boot_key: VerifiedBootKey,
    pub device_locked: bool,
    pub verified_boot_state: VerifiedBootState,
    pub verified_boot_hash: Option<VerifiedBootHash>,
}

impl TryFrom<asn::RootOfTrustV1V2<'_>> for BoundedRootOfTrust {
    type Error = ();

    fn try_from(data: asn::RootOfTrustV1V2) -> Result<Self, Self::Error> {
        Ok(BoundedRootOfTrust {
            verified_boot_key: VerifiedBootKey::try_from(data.verified_boot_key.to_vec())
                .map_err(|_| ())?,
            device_locked: data.device_locked,
            verified_boot_state: data.verified_boot_state.into(),
            verified_boot_hash: None,
        })
    }
}

impl TryFrom<asn::RootOfTrust<'_>> for BoundedRootOfTrust {
    type Error = ();

    fn try_from(data: asn::RootOfTrust) -> Result<Self, Self::Error> {
        Ok(BoundedRootOfTrust {
            verified_boot_key: VerifiedBootKey::try_from(data.verified_boot_key.to_vec())
                .map_err(|_| ())?,
            device_locked: data.device_locked,
            verified_boot_state: data.verified_boot_state.into(),
            verified_boot_hash: Some(
                VerifiedBootHash::try_from(data.verified_boot_hash.to_vec()).map_err(|_| ())?,
            ),
        })
    }
}

#[derive(
    RuntimeDebug,
    Encode,
    Decode,
    MaxEncodedLen,
    TypeInfo,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
)]
pub enum VerifiedBootState {
    Verified,
    SelfSigned,
    Unverified,
    Failed,
}

impl From<asn::VerifiedBootState> for VerifiedBootState {
    fn from(data: asn::VerifiedBootState) -> Self {
        match data.value() {
            0 => VerifiedBootState::Verified,
            1 => VerifiedBootState::SelfSigned,
            2 => VerifiedBootState::Unverified,
            _ => VerifiedBootState::Failed,
        }
    }
}

#[derive(
    RuntimeDebug,
    Encode,
    Decode,
    MaxEncodedLen,
    TypeInfo,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
)]
pub struct BoundedAttestationApplicationId {
    pub package_infos: PackageInfoSet,
    pub signature_digests: SignatureDigestSet,
}

impl<'a> TryFrom<asn::AttestationApplicationId<'a>> for BoundedAttestationApplicationId {
    type Error = ();

    fn try_from(value: asn::AttestationApplicationId<'a>) -> Result<Self, Self::Error> {
        Ok(Self {
            package_infos: value
                .package_infos
                .map(|package_info| BoundedAttestationPackageInfo::try_from(package_info))
                .collect::<Result<Vec<BoundedAttestationPackageInfo>, Self::Error>>()?
                .try_into()
                .map_err(|_| ())?,
            signature_digests: value
                .signature_digests
                .map(|digest| Digest::try_from(digest.to_vec()))
                .collect::<Result<Vec<Digest>, Vec<u8>>>()
                .map_err(|_| ())?
                .try_into()
                .map_err(|_| ())?,
        })
    }
}

#[derive(
    RuntimeDebug,
    Encode,
    Decode,
    MaxEncodedLen,
    TypeInfo,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
)]
pub struct BoundedAttestationPackageInfo {
    pub package_name: PackageName,
    pub version: i64,
}

impl<'a> TryFrom<asn::AttestationPackageInfo<'a>> for BoundedAttestationPackageInfo {
    type Error = ();

    fn try_from(value: asn::AttestationPackageInfo<'a>) -> Result<Self, Self::Error> {
        Ok(Self {
            package_name: value.package_name.to_vec().try_into().map_err(|_| ())?,
            version: value.version,
        })
    }
}
