//! Secret key tests

#![cfg(feature = "dev")]

use elliptic_curve_vendored::dev::SecretKey;

#[test]
fn undersize_secret_key() {
    assert!(SecretKey::from_be_bytes(&[]).is_err());
}
