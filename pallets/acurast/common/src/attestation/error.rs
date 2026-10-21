use asn1::ParseError;
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;

#[derive(Debug, Encode, Decode, TypeInfo, Clone, PartialEq, Eq)]

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
	/// A non-leaf certificate is missing the BasicConstraints extension.
	BasicConstraintsMissing,
	/// A non-leaf certificate is not marked as a certificate authority (cA = FALSE).
	NotACertificateAuthority,
	/// The pathLenConstraint of a CA certificate is exceeded by the chain.
	PathLenConstraintViolated,
	/// A non-leaf certificate is missing the KeyUsage extension.
	KeyUsageMissing,
	/// A non-leaf certificate is not allowed to sign certificates (keyCertSign not set).
	KeyCertSignNotAllowed,
	/// A certificate validity date is invalid or before the unix epoch.
	InvalidCertificateDate,
	/// A certificate is not yet valid or has expired.
	CertificateNotValidAtTime,
	/// The chain does not have the shape of a genuine Android key attestation chain.
	UnexpectedChainShape,
	/// The leaf certificate's serial number is not 1.
	UnexpectedLeafSerialNumber,
	/// The RKP provisioning info extension could not be parsed.
	ParseProvisioningInfo,
	/// The RKP provisioning info does not mark the device's build as approved.
	BuildNotApproved,
	/// The RKP provisioning info reports too many certificates issued to the device or lacks their count.
	TooManyCertificatesIssued,
	/// The leaf's verified boot hash does not match the vbmeta digest in the RKP provisioning info, or either is missing.
	VerifiedBootHashMismatch,
	/// The attestation could not be extracted from the leaf certificate.
	ExtractAttestation,
	/// The key's security levels disagree, are software, or do not match what the chain claims.
	SecurityLevelMismatch,
	/// An Android attestation does not chain to a Google root, or an Apple one to the Apple root.
	AttestationRootMismatch,
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
