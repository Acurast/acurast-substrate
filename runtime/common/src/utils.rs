use core::marker::PhantomData;

use frame_support::{
	traits::IsType,
	weights::constants::{ExtrinsicBaseWeight, WEIGHT_REF_TIME_PER_SECOND},
};
use pallet_acurast::{
	Attestation, BoundedAttestationContent, BoundedDeviceAttestation, BoundedKeyDescription,
	VerifiedBootState,
};
use pallet_acurast_processor_manager::ProcessorPairingFor;
use sp_runtime::traits::{CheckedAdd, IdentifyAccount, Verify};
use sp_std::prelude::*;

use crate::{constants::MILLIUNIT, types::Balance};

/// Returns the base transaction fee.
pub fn base_tx_fee() -> Balance {
	MILLIUNIT / 10
}

/// Returns the default fee per second.
pub fn default_fee_per_second() -> u128 {
	let base_weight = Balance::from(ExtrinsicBaseWeight::get().ref_time());
	let base_tx_per_second = (WEIGHT_REF_TIME_PER_SECOND as u128) / base_weight;
	base_tx_per_second * base_tx_fee()
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
	if let Some(bundle_id) = &device_attestation.key_usage_properties.t1204 {
		return allowed_bundle_ids.contains(&bundle_id.as_slice());
	}
	false
}

pub trait FeePayerProvider<T: frame_system::Config> {
	fn fee_payer(account: &T::AccountId, call: &T::RuntimeCall) -> T::AccountId;
}

pub trait PairingProvider<T: pallet_acurast_processor_manager::Config> {
	fn pairing_for_call(call: &T::RuntimeCall) -> Option<(&ProcessorPairingFor<T>, bool)>;
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct FeePayer<T: pallet_acurast_processor_manager::Config, P: PairingProvider<T>>(
	PhantomData<(T, P)>,
);

impl<T: pallet_acurast_processor_manager::Config, P: PairingProvider<T>> FeePayerProvider<T> for FeePayer<T, P> where <T as frame_system::Config>::AccountId: IsType<<<<T as pallet_acurast_processor_manager::Config>::Proof as Verify>::Signer as IdentifyAccount>::AccountId> {
    fn fee_payer(account: &<T as frame_system::Config>::AccountId, call: &<T as frame_system::Config>::RuntimeCall) -> <T as frame_system::Config>::AccountId {
        let mut manager = pallet_acurast_processor_manager::Pallet::<T>::manager_for_processor(account);

		if manager.is_none() {
			if let Some((pairing, is_multi)) = P::pairing_for_call(call) {
				if pairing.validate_timestamp::<T>() {
					let is_valid = if is_multi {
						pairing.multi_validate_signature::<T>(&pairing.account)
					} else {
						let counter =
							pallet_acurast_processor_manager::Pallet::<T>::counter_for_manager(
								&pairing.account,
							)
							.unwrap_or(0u8.into())
							.checked_add(&1u8.into());
						if let Some(counter) = counter {
							pairing.validate_signature::<T>(&pairing.account, counter)
						} else {
							false
						}
					};
					if is_valid {
						manager = Some(pairing.account.clone());
					}
				}
			}
		}

		manager.unwrap_or(account.clone())
    }
}
