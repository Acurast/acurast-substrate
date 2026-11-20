use crate::{MAX_HOST_LENGTH, MAX_LABEL_LENGTH, MIN_LABEL_LENGTH};

/// Checks one DNS label: `min_len`-63 chars from `[a-z0-9-]`, no leading or trailing `-`.
fn is_valid_dns_label(label: &[u8], min_len: u32) -> bool {
	(min_len..=MAX_LABEL_LENGTH).contains(&(label.len() as u32))
		&& label.iter().all(|c| matches!(c, b'a'..=b'z' | b'0'..=b'9' | b'-'))
		&& label.first() != Some(&b'-')
		&& label.last() != Some(&b'-')
}

/// Checks the format rules for a claimable label; the reserved and banned lists are checked separately.
pub fn is_valid_label(label: &[u8]) -> bool {
	is_valid_dns_label(label, MIN_LABEL_LENGTH)
		&& !label.starts_with(b"xn--")
		&& !(label.len() == 16 && label.iter().all(u8::is_ascii_hexdigit))
}

/// Checks that `host` is a lowercase DNS name: dot-separated labels of 1-63 chars from `[a-z0-9-]`.
pub fn is_valid_host(host: &[u8]) -> bool {
	!host.is_empty()
		&& host.len() as u32 <= MAX_HOST_LENGTH
		&& host.split(|c| *c == b'.').all(|label| is_valid_dns_label(label, 1))
}
