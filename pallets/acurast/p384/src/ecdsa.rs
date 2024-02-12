//! Elliptic Curve Digital Signature Algorithm (ECDSA)
//!
//! This module contains support for computing and verifying ECDSA signatures.
//! To use it, you will need to enable one of the two following Cargo features:
//!
//! - `ecdsa-core`: provides only the [`Signature`] type (which represents an
//!   ECDSA/P-384 signature). Does not require the `arithmetic` feature. This is
//!   useful for 3rd-party crates which wish to use the `Signature` type for
//!   interoperability purposes (particularly in conjunction with the
//!   [`signature::Signer`] trait. Example use cases for this include other
//!   software implementations of ECDSA/P-384 and wrappers for cloud KMS
//!   services or hardware devices (HSM or crypto hardware wallet).
//! - `ecdsa`: provides `ecdsa-core` features plus the [`SigningKey`] and
//!   [`VerifyingKey`] types which natively implement ECDSA/P-384 signing and
//!   verification.
//!
pub use ecdsa::signature_vendored::{self as signature, Error};
pub use ecdsa_vendored as ecdsa;
#[cfg(feature = "ecdsa")]
use {
    crate::{AffinePoint, Scalar},
    ecdsa::hazmat::{SignPrimitive, VerifyPrimitive},
};

use super::NistP384;

/// ECDSA/P-384 signature (fixed-size)
pub type Signature = ecdsa::Signature<NistP384>;

/// ECDSA/P-384 signature (ASN.1 DER encoded)
pub type DerSignature = ecdsa::der::Signature<NistP384>;

/// ECDSA/P-384 signing key
#[cfg(feature = "ecdsa")]
#[cfg_attr(docsrs, doc(cfg(feature = "ecdsa")))]
pub type SigningKey = ecdsa::SigningKey<NistP384>;

/// ECDSA/P-384 verification key (i.e. public key)
#[cfg(feature = "ecdsa")]
#[cfg_attr(docsrs, doc(cfg(feature = "ecdsa")))]
pub type VerifyingKey = ecdsa::VerifyingKey<NistP384>;

#[cfg(feature = "sha384")]
#[cfg_attr(docsrs, doc(cfg(feature = "sha384")))]
impl ecdsa::hazmat::DigestPrimitive for NistP384 {
    type Digest = sha2::Sha384;
}

#[cfg(feature = "ecdsa")]
impl SignPrimitive<NistP384> for Scalar {}

#[cfg(feature = "ecdsa")]
impl VerifyPrimitive<NistP384> for AffinePoint {}

#[cfg(all(test, feature = "ecdsa"))]
mod tests {
    use crate::{ecdsa::SigningKey, SecretKey};

    #[test]
    fn signing_secret_key_equivalent() {
        let raw_sk: [u8; 48] = [
            32, 52, 118, 9, 96, 116, 119, 172, 168, 251, 251, 197, 230, 33, 132, 85, 243, 25, 150,
            105, 121, 46, 248, 180, 102, 250, 168, 123, 220, 103, 121, 129, 68, 200, 72, 221, 3,
            102, 30, 237, 90, 198, 36, 97, 52, 12, 234, 150,
        ];

        let sigk = SigningKey::from_bytes(raw_sk.as_slice()).unwrap();
        let seck = SecretKey::from_be_bytes(raw_sk.as_slice()).unwrap();

        assert_eq!(sigk.to_bytes().as_slice(), &raw_sk);
        assert_eq!(sigk.to_bytes(), seck.to_be_bytes());
    }

    mod sign {
        use crate::{test_vectors::ecdsa::ECDSA_TEST_VECTORS, NistP384};
        ecdsa_vendored::new_signing_test!(NistP384, ECDSA_TEST_VECTORS);
    }

    mod verify {
        use crate::{test_vectors::ecdsa::ECDSA_TEST_VECTORS, NistP384};
        ecdsa_vendored::new_verification_test!(NistP384, ECDSA_TEST_VECTORS);
    }
}
