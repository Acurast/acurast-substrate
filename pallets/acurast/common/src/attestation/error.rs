#![cfg_attr(all(feature = "alloc", not(feature = "std"), not(test)), no_std)]

use asn1::ParseError;
use codec::{Decode, Encode};
use frame_support::pallet_prelude::RuntimeDebug;
use scale_info::TypeInfo;

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq, Eq)]

pub enum ValidationError {
    /// Error occured while parsing the key description
    ParseKeyDescription,
    /// The certificate chain is too short
    ChainTooShort,
    /// The certificate chain is too long
    ChainTooLong,
    /// Generic decode error
    DecodeError,
    /// Generic parse error
    ParseError,
    /// The root certificate is not trusted
    UntrustedRoot,
    /// Missing extension field in certificate
    ExtensionMissing,
    /// Error occured when parsing the extension field
    ParseExtension,
    /// Attestation version is not supported
    UnsupportedAttestationVersion(i64),
    /// Error occured while parsing the P256 public key
    ParseP256PublicKey,
    /// Error occured while parsing the P384 public key
    ParseP384PublicKey,
    /// ECDSA Algorithm missing
    MissingECDSAAlgorithmTyp,
    /// Public key missing
    MissingPublicKey,
    /// Signature has an invalid encoding
    InvalidSignatureEncoding,
    /// Signature is invalid
    InvalidSignature,
    /// Signature Algorithm is not supported
    UnsupportedSignatureAlgorithm,
    /// Public Key Algorithm is not supported
    UnsupportedPublicKeyAlgorithm,
    /// Issuer is invalid
    InvalidIssuer,
    /// Specified signature algorithms do not match.
    ///
    /// The signature field in the sequence
    /// [tbsCertificate](https://www.rfc-editor.org/rfc/rfc5280#section-4.1.2.3)
    /// MUST contain the same algorithm identifier as the signatureAlgorithm
    /// field in the sequence
    /// [Certificate](https://www.rfc-editor.org/rfc/rfc5280#section-4.1.1.2).
    SignatureMismatch,
}

impl From<ParseError> for ValidationError {
    fn from(_: ParseError) -> Self {
        Self::ParseExtension
    }
}

impl From<p384::elliptic_curve::Error> for ValidationError {
    fn from(_: p384::elliptic_curve::Error) -> Self {
        Self::ParseP384PublicKey
    }
}
