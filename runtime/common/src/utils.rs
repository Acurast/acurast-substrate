use pallet_acurast::{
	Attestation, AttestationSecurityLevel, BoundedAttestationContent, BoundedDeviceAttestation,
	BoundedKeyDescription, IsFundableCall, VerifiedBootState,
};
use pallet_acurast_processor_manager::{Config as ProcessorManagerConfig, OnboardingProvider};
use sp_std::prelude::*;

/// Resolves the account that pays the fee for `call` submitted by `who`.
pub fn fee_payer<Runtime, P, OP>(
	who: &Runtime::AccountId,
	call: &Runtime::RuntimeCall,
) -> Runtime::AccountId
where
	Runtime: ProcessorManagerConfig,
	P: IsFundableCall<Runtime::RuntimeCall>,
	OP: OnboardingProvider<Runtime>,
{
	if P::is_manager_fundable_call(call) {
		OP::fee_payer(who, call)
	} else {
		who.clone()
	}
}

pub fn check_attestation(
	attestation: &Attestation,
	allowed_package_names: &[&[u8]],
	allowed_signature_digests: &[&[u8]],
	allowed_bundle_ids: &[&[u8]],
) -> bool {
	match &attestation.content {
		BoundedAttestationContent::KeyDescription(key_description) => {
			check_key_description(key_description, allowed_package_names, allowed_signature_digests)
		},
		BoundedAttestationContent::DeviceAttestation(device_attestation) => {
			check_device_attestation(device_attestation, allowed_bundle_ids)
		},
	}
}

pub fn check_key_description(
	key_description: &BoundedKeyDescription,
	allowed_package_names: &[&[u8]],
	allowed_signature_digests: &[&[u8]],
) -> bool {
	let root_of_trust = &key_description.tee_enforced.root_of_trust;
	if let Some(root_of_trust) = root_of_trust {
		if root_of_trust.verified_boot_state != VerifiedBootState::Verified
			|| !root_of_trust.device_locked
		{
			return false;
		}
	} else {
		return false;
	}

	if !matches!(
		key_description.attestation_security_level,
		AttestationSecurityLevel::StrongBox | AttestationSecurityLevel::TrustedEnvironemnt,
	) {
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
		let is_package_name_allowed = !package_names.is_empty()
			&& package_names
				.iter()
				.all(|package_name| allowed_package_names.contains(package_name));
		if is_package_name_allowed {
			let signature_digests = attestation_application_id
				.signature_digests
				.iter()
				.map(|signature_digest| signature_digest.as_slice())
				.collect::<Vec<_>>();
			result = !signature_digests.is_empty()
				&& signature_digests
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
	if let Some(bundle_id) = &device_attestation.key_usage_properties.t1204 {
		return allowed_bundle_ids.contains(&bundle_id.as_slice());
	}
	false
}

#[cfg(test)]
mod tests {
	use super::*;
	use pallet_acurast::{
		BoundedAttestationApplicationId, BoundedAttestationPackageInfo, BoundedAuthorizationList,
		BoundedRootOfTrust,
	};
	use parity_scale_codec::Decode;

	const PACKAGE: &[u8] = b"com.acurast.attested.executor";
	const DIGEST: &[u8] = &[1; 32];

	fn key_description(packages: &[&[u8]], digests: &[&[u8]]) -> BoundedKeyDescription {
		let mut tee_enforced = BoundedAuthorizationList::decode(&mut &[0u8; 64][..]).unwrap();
		tee_enforced.root_of_trust = Some(BoundedRootOfTrust {
			verified_boot_key: Default::default(),
			device_locked: true,
			verified_boot_state: VerifiedBootState::Verified,
			verified_boot_hash: None,
		});
		tee_enforced.attestation_application_id = Some(BoundedAttestationApplicationId {
			package_infos: packages
				.iter()
				.map(|p| BoundedAttestationPackageInfo {
					package_name: p.to_vec().try_into().unwrap(),
					version: 1,
				})
				.collect::<Vec<_>>()
				.try_into()
				.unwrap(),
			signature_digests: digests
				.iter()
				.map(|d| d.to_vec().try_into().unwrap())
				.collect::<Vec<_>>()
				.try_into()
				.unwrap(),
		});
		BoundedKeyDescription {
			attestation_security_level: AttestationSecurityLevel::TrustedEnvironemnt,
			key_mint_security_level: AttestationSecurityLevel::TrustedEnvironemnt,
			software_enforced: tee_enforced.clone(),
			tee_enforced,
		}
	}

	#[test]
	fn check_key_description_rejects_empty_sets() {
		let check = |p: &[&[u8]], d: &[&[u8]]| {
			check_key_description(&key_description(p, d), &[PACKAGE], &[DIGEST])
		};
		assert!(check(&[PACKAGE], &[DIGEST]));
		assert!(!check(&[], &[DIGEST]));
		assert!(!check(&[PACKAGE], &[]));
		assert!(!check(&[], &[]));
	}
}
