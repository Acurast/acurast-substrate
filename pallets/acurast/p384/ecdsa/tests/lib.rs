//! Smoke tests which use `MockCurve`

#![cfg(feature = "dev")]

use elliptic_curve::dev::MockCurve;

type Signature = ecdsa_vendored::Signature<MockCurve>;
type SignatureBytes = ecdsa_vendored::SignatureBytes<MockCurve>;

#[test]
fn rejects_all_zero_signature() {
    let all_zero_bytes = SignatureBytes::default();
    assert!(Signature::try_from(all_zero_bytes.as_ref()).is_err());
}
