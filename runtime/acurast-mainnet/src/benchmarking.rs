use acurast_runtime_common::types::{ExtraFor, Signature};
use frame_benchmarking::{account, define_benchmarks};
use frame_support::{assert_ok, traits::tokens::currency::Currency};
use pallet_acurast::{
	Attestation, AttestationValidity, BoundedAttestationContent, BoundedDeviceAttestation,
	BoundedDeviceAttestationDeviceOSInformation, BoundedDeviceAttestationKeyUsageProperties,
	BoundedDeviceAttestationNonce, JobModules, PoolId, StoredAttestation,
};
use pallet_acurast_marketplace::{
	Advertisement, AssignmentStrategy, JobRequirements, PlannedExecution, Pricing, SchedulingWindow,
};
use sp_core::crypto::UncheckedFrom;
use sp_std::vec;

use crate::{AcurastMarketplace, Balance, Balances, BundleId, Runtime};

define_benchmarks!(
	// TODO uncomment with fixed version of cumulus-pallet-parachain-system that includes PR https://github.com/paritytech/cumulus/pull/2766/files
	// [frame_system, SystemBench::<Runtime>]
	// [pallet_balances, Balances]
	// [pallet_session, SessionBench::<Runtime>]
	// [pallet_timestamp, Timestamp]
	// [pallet_collator_selection, CollatorSelection]
	// [cumulus_pallet_xcmp_queue, XcmpQueue]
	[pallet_acurast, Acurast]
	[pallet_acurast_processor_manager, AcurastProcessorManager]
	[pallet_acurast_fee_manager, AcurastFeeManager]
	[pallet_acurast_marketplace, AcurastMarketplace]
	// [pallet_acurast_hyperdrive, AcurastHyperdrive]
);

fn create_funded_user(
	string: &'static str,
	n: u32,
	amount: Balance,
) -> <Runtime as frame_system::Config>::AccountId {
	const SEED: u32 = 0;
	let user = account(string, n, SEED);
	Balances::make_free_balance_be(&user, amount);
	let _ = Balances::issue(amount);
	return user;
}

pub struct AcurastBenchmarkHelper;

impl pallet_acurast::BenchmarkHelper<Runtime> for AcurastBenchmarkHelper {
	fn registration_extra(instant_match: bool) -> ExtraFor<Runtime> {
		let processor = Self::funded_account(0);
		let ad = Advertisement {
			pricing: Pricing {
				fee_per_millisecond: 1,
				fee_per_storage_byte: 1,
				base_fee_per_execution: 1,
				scheduling_window: SchedulingWindow::End(4133977199000),
			},
			allowed_consumers: None,
			storage_capacity: 100_000,
			max_memory: 100_000,
			network_request_quota: 100,
			available_modules: JobModules::default(),
		};
		assert_ok!(AcurastMarketplace::do_advertise(&processor, &ad));
		ExtraFor::<Runtime> {
			requirements: JobRequirements {
				slots: 1,
				reward: 2_000_000_000,
				min_reputation: None,
				assignment_strategy: AssignmentStrategy::Single(if instant_match {
					Some(
						vec![PlannedExecution { source: Self::funded_account(0), start_delay: 0 }]
							.try_into()
							.unwrap(),
					)
				} else {
					None
				}),
				processor_version: None,
				runtime: pallet_acurast_marketplace::Runtime::NodeJS,
			},
		}
	}

	fn funded_account(index: u32) -> <Runtime as frame_system::Config>::AccountId {
		create_funded_user("pallet_acurast", index, 1 << 60)
	}
}

impl pallet_acurast_marketplace::BenchmarkHelper<Runtime> for AcurastBenchmarkHelper {
	fn registration_extra(
		r: pallet_acurast_marketplace::JobRequirementsFor<Runtime>,
	) -> <Runtime as pallet_acurast_marketplace::Config>::RegistrationExtra {
		ExtraFor::<Runtime> { requirements: r }
	}

	fn funded_account(
		index: u32,
		amount: <Runtime as pallet_acurast_marketplace::Config>::Balance,
	) -> <Runtime as frame_system::Config>::AccountId {
		create_funded_user("pallet_acurast_marketplace", index, amount)
	}
}

impl pallet_acurast_processor_manager::BenchmarkHelper<Runtime> for AcurastBenchmarkHelper {
	fn dummy_proof() -> <Runtime as pallet_acurast_processor_manager::Config>::Proof {
		Signature::Sr25519(sp_core::sr25519::Signature::unchecked_from([0u8; 64]))
	}

	fn advertisement() -> <Runtime as pallet_acurast_processor_manager::Config>::Advertisement {
		Advertisement {
			pricing: Pricing {
				fee_per_millisecond: 1,
				fee_per_storage_byte: 1,
				base_fee_per_execution: 1,
				scheduling_window: SchedulingWindow::End(4133977199000),
			},
			allowed_consumers: None,
			storage_capacity: 100_000,
			max_memory: 100_000,
			network_request_quota: 100,
			available_modules: JobModules::default(),
		}
	}

	fn funded_account(index: u32) -> <Runtime as frame_system::Config>::AccountId {
		create_funded_user("pallet_acurast", index, 1 << 60)
	}

	fn attest_account(account: &<Runtime as frame_system::Config>::AccountId) {
		let attestation = Attestation {
			cert_ids: Default::default(),
			content: BoundedAttestationContent::DeviceAttestation(BoundedDeviceAttestation {
				key_usage_properties: BoundedDeviceAttestationKeyUsageProperties {
					t4: None,
					t1200: None,
					t1201: None,
					t1202: None,
					t1203: None,
					t1204: Some(BundleId::get().to_vec().try_into().unwrap()),
					t5: None,
					t1206: None,
					t1207: None,
					t1209: None,
					t1210: None,
					t1211: None,
				},
				device_os_information: BoundedDeviceAttestationDeviceOSInformation {
					t1400: None,
					t1104: None,
					t1403: None,
					t1420: None,
					t1026: None,
					t1029: None,
				},
				nonce: BoundedDeviceAttestationNonce { nonce: None },
			}),
			validity: AttestationValidity { not_before: 0, not_after: u64::MAX },
		};
		<StoredAttestation<Runtime>>::insert(account, attestation);
	}

	fn create_compute_pool() -> PoolId {
		panic!("pallet_acurast_compute not yet installed for this runtime");
	}
}
