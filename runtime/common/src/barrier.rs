use core::marker::PhantomData;

use frame_support::traits::Get;
use pallet_acurast::{Attestation, BoundedAttestationContent, ProcessorType};
use sp_std::prelude::*;

use crate::utils::{check_attestation, check_key_description};

pub struct Barrier<
	Runtime,
	PackageNames,
	SignatureDigests,
	CorePackageNames,
	LitePackageNames,
	CoreSignatureDigests,
	LiteSignatureDigests,
	BundleIds,
> {
	#[allow(clippy::type_complexity)]
	_phantom_data: PhantomData<(
		Runtime,
		PackageNames,
		SignatureDigests,
		CorePackageNames,
		LitePackageNames,
		CoreSignatureDigests,
		LiteSignatureDigests,
		BundleIds,
	)>,
}

impl<
		Runtime,
		PackageNames,
		SignatureDigests,
		CorePackageNames,
		CoreSignatureDigests,
		LitePackageNames,
		LiteSignatureDigests,
		BundleIds,
	> pallet_acurast::KeyAttestationBarrier<Runtime>
	for Barrier<
		Runtime,
		PackageNames,
		SignatureDigests,
		CorePackageNames,
		LitePackageNames,
		CoreSignatureDigests,
		LiteSignatureDigests,
		BundleIds,
	>
where
	Runtime: frame_system::Config + pallet_acurast::Config,
	PackageNames: Get<Vec<&'static [u8]>>,
	SignatureDigests: Get<Vec<&'static [u8]>>,
	CorePackageNames: Get<Vec<&'static [u8]>>,
	CoreSignatureDigests: Get<Vec<&'static [u8]>>,
	LitePackageNames: Get<Vec<&'static [u8]>>,
	LiteSignatureDigests: Get<Vec<&'static [u8]>>,
	BundleIds: Get<Vec<&'static [u8]>>,
{
	fn accept_attestation_for_origin(
		_origin: &<Runtime as frame_system::Config>::AccountId,
		attestation: &Attestation,
	) -> bool {
		check_attestation(
			attestation,
			PackageNames::get().as_slice(),
			SignatureDigests::get().as_slice(),
			BundleIds::get().as_slice(),
		)
	}

	fn check_attestation_is_of_type(
		attestation: &Attestation,
		processor_type: ProcessorType,
	) -> bool {
		match processor_type {
			ProcessorType::Core => match &attestation.content {
				BoundedAttestationContent::KeyDescription(key_description) => {
					check_key_description(
						key_description,
						CorePackageNames::get().as_slice(),
						CoreSignatureDigests::get().as_slice(),
					)
				},
				BoundedAttestationContent::DeviceAttestation(_) => false,
			},
			ProcessorType::Lite => match &attestation.content {
				BoundedAttestationContent::KeyDescription(key_description) => {
					check_key_description(
						key_description,
						LitePackageNames::get().as_slice(),
						LiteSignatureDigests::get().as_slice(),
					)
				},
				BoundedAttestationContent::DeviceAttestation(_) => true,
			},
		}
	}
}
