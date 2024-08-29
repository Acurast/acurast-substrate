use crate::{AcurastMarketplace, Balance, Balances, ExtraFor, MultiSignature, Runtime};
use frame_benchmarking::account;
use frame_support::{assert_ok, traits::tokens::currency::Currency};
use pallet_acurast::JobModules;
use pallet_acurast_marketplace::{
	Advertisement, AssignmentStrategy, JobRequirements, PlannedExecution, Pricing, SchedulingWindow,
};
use sp_core::crypto::UncheckedFrom;
use sp_std::vec;

fn create_funded_user(
	string: &'static str,
	n: u32,
	amount: Balance,
) -> <Runtime as frame_system::Config>::AccountId {
	const SEED: u32 = 0;
	let user = account(string, n, SEED);
	Balances::make_free_balance_be(&user, amount);
	let _ = Balances::issue(amount);
	return user
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
		MultiSignature::Sr25519(sp_core::sr25519::Signature::unchecked_from([0u8; 64]))
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
}
