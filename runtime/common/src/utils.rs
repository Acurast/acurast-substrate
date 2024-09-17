use pallet_acurast::{
	Attestation, BoundedAttestationContent, BoundedDeviceAttestation, BoundedKeyDescription,
	VerifiedBootState,
};
use sp_std::prelude::*;

pub fn check_attestation(
	attestation: &Attestation,
	allowed_package_names: &[&[u8]],
	allowed_signature_digests: &[&[u8]],
	allowed_bundle_ids: &[&[u8]],
) -> bool {
	match &attestation.content {
		BoundedAttestationContent::KeyDescription(key_description) =>
			check_key_description(key_description, allowed_package_names, allowed_signature_digests),
		BoundedAttestationContent::DeviceAttestation(device_attestation) =>
			check_device_attestation(device_attestation, allowed_bundle_ids),
	}
}

pub fn check_key_description(
	key_description: &BoundedKeyDescription,
	allowed_package_names: &[&[u8]],
	allowed_signature_digests: &[&[u8]],
) -> bool {
	let root_of_trust = &key_description.tee_enforced.root_of_trust;
	if let Some(root_of_trust) = root_of_trust {
		if root_of_trust.verified_boot_state != VerifiedBootState::Verified {
			return false;
		}
	} else {
		return false;
	}
	let mut result = false;
	let attestation_application_id = key_description
		.tee_enforced
		.attestation_application_id
		.as_ref()
		.or(key_description.software_enforced.attestation_application_id.as_ref());

	if let Some(attestation_application_id) = attestation_application_id {
		let package_names = attestation_application_id
			.package_infos
			.iter()
			.map(|package_info| package_info.package_name.as_slice())
			.collect::<Vec<_>>();
		let is_package_name_allowed = package_names
			.iter()
			.all(|package_name| allowed_package_names.contains(package_name));
		if is_package_name_allowed {
			let signature_digests = attestation_application_id
				.signature_digests
				.iter()
				.map(|signature_digest| signature_digest.as_slice())
				.collect::<Vec<_>>();
			result = signature_digests
				.iter()
				.all(|digest| allowed_signature_digests.contains(digest));
		}
	}
	result
}

pub fn check_device_attestation(
	device_attestation: &BoundedDeviceAttestation,
	allowed_bundle_ids: &[&[u8]],
) -> bool {
	allowed_bundle_ids.contains(&device_attestation.key_usage_properties.t1204.as_slice())
}
