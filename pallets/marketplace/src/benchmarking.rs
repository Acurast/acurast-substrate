use frame_benchmarking::{benchmarks, whitelist_account};
use frame_support::{
	assert_ok,
	sp_runtime::{
		traits::{IdentifyAccount, Verify},
		DispatchError, Perquintill,
	},
	traits::IsType,
};
use frame_system::RawOrigin;
use sp_core::*;
use sp_std::prelude::*;

use crate::Config;
use pallet_acurast::{
	ComputeHooks, JobId, JobIdSequence, JobModules, JobRegistrationFor, MultiOrigin,
	Pallet as Acurast, Schedule, Script,
};
use pallet_acurast_compute::Pallet as AcurastCompute;

pub use crate::stub::*;
use crate::Pallet as AcurastMarketplace;

use super::*;

pub trait BenchmarkHelper<T: Config> {
	/// Extends the job requirements, defined by benchmarking code in this pallet, with the containing struct RegistrationExtra.
	fn registration_extra(r: JobRequirementsFor<T>) -> <T as Config>::RegistrationExtra;
	fn funded_account(index: u32, amount: T::Balance) -> T::AccountId;
	fn remove_job_registration(job_id: &JobId<T::AccountId>);
}

pub fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

pub fn advertisement<T: Config>(
	fee_per_millisecond: u128,
	storage_capacity: u32,
) -> AdvertisementFor<T> {
	Advertisement {
		pricing: Pricing {
			fee_per_millisecond: fee_per_millisecond.into(),
			fee_per_storage_byte: 5u8.into(),
			base_fee_per_execution: 0u8.into(),
			scheduling_window: SchedulingWindow::End(4133977199000),
		},
		allowed_consumers: None,
		storage_capacity,
		max_memory: 100_000,
		network_request_quota: 100,
		available_modules: JobModules::default(),
	}
}

const DAY: u64 = 1000 * 60 * 60 * 24;

pub fn job_registration_with_reward<T: Config>(
	script: Script,
	slots: u8,
	duration: u64,
	reward_value: u128,
	memory: u32,
	network_requests: u32,
	storage: u32,
	schedule_shift: Option<u64>,
	instant_match_processor: Option<PlannedExecution<T::AccountId>>,
) -> JobRegistrationFor<T> {
	let reward: <T as Config>::Balance = reward_value.into();
	let r = JobRequirements {
		slots,
		reward,
		min_reputation: Some(0),
		assignment_strategy: AssignmentStrategy::Single(
			instant_match_processor.map(|m| vec![m].try_into().unwrap()),
		),
		processor_version: None,
		runtime: Runtime::NodeJS,
	};
	let r: <T as Config>::RegistrationExtra = <T as Config>::BenchmarkHelper::registration_extra(r);
	let r: <T as pallet_acurast::Config>::RegistrationExtra = r.into();
	let schedule_shift = schedule_shift.unwrap_or(0);
	JobRegistrationFor::<T> {
		script,
		allowed_sources: None,
		allow_only_verified_sources: false,
		schedule: Schedule {
			duration,
			start_time: 1689332400000 + (DAY * schedule_shift), // 30.12.2050 13:00
			end_time: 1689418800000 + (DAY * schedule_shift),   // 31.12.2050 13:00 (one day later)
			interval: 180000,                                   // 30min
			max_start_delay: 5000,
		},
		memory,
		network_requests,
		storage,
		required_modules: JobModules::default(),
		extra: r,
	}
}

pub fn competing_job_registration_with_reward<T: Config>(
	script: Script,
	slots: u8,
	duration: u64,
	reward_value: u128,
) -> JobRegistrationFor<T> {
	let reward: <T as Config>::Balance = reward_value.into();
	let r = JobRequirements {
		slots,
		reward,
		min_reputation: Some(0),
		assignment_strategy: AssignmentStrategy::Competing,
		processor_version: None,
		runtime: Runtime::NodeJS,
	};
	let r: <T as Config>::RegistrationExtra = <T as Config>::BenchmarkHelper::registration_extra(r);
	let r: <T as pallet_acurast::Config>::RegistrationExtra = r.into();
	JobRegistrationFor::<T> {
		script,
		allowed_sources: None,
		allow_only_verified_sources: false,
		schedule: Schedule {
			duration,
			start_time: 1689332400000, // 30.12.2050 13:00
			end_time: 1689418800000,   // 31.12.2050 13:00 (one day later)
			interval: 1800000,         // 30min
			max_start_delay: 5000,
		},
		memory: 1_000u32,
		network_requests: 1,
		storage: 1_000u32,
		required_modules: JobModules::default(),
		extra: r,
	}
}

pub fn script() -> Script {
	SCRIPT_BYTES.to_vec().try_into().unwrap()
}

fn advertise_helper<T: Config>(
	account_index: u32,
	submit: bool,
) -> (T::AccountId, AdvertisementFor<T>)
where
	T: pallet_balances::Config + pallet_acurast_compute::Config,
{
	let caller: T::AccountId =
		<T as Config>::BenchmarkHelper::funded_account(account_index, u32::MAX.into());
	whitelist_account!(caller);

	let ad = advertisement::<T>(1, 100_000);

	if submit {
		let register_call = AcurastMarketplace::<T>::advertise(
			RawOrigin::Signed(caller.clone()).into(),
			ad.clone(),
		);
		assert_ok!(register_call);
		let _ = AcurastCompute::<T>::commit(
			&caller,
			vec![(1, 1, 2), (2, 1, 2), (3, 1, 2), (4, 1, 2), (5, 1, 2), (6, 1, 2)],
		);
	}

	(caller, ad)
}

fn register_helper<T: Config>(
	account_index: u32,
	slots: u8,
) -> (T::AccountId, JobRegistrationFor<T>)
where
	T: pallet_balances::Config,
{
	let caller: T::AccountId =
		<T as Config>::BenchmarkHelper::funded_account(account_index, u32::MAX.into());
	whitelist_account!(caller);

	let job =
		job_registration_with_reward::<T>(script(), slots, 500, 20100, 1000, 1, 1000, None, None);

	(caller, job)
}

fn setup_pools<T: pallet_acurast_compute::Config>() {
	assert_ok!(AcurastCompute::<T>::create_pool(
		RawOrigin::Root.into(),
		*b"v1_cpu_single_core______",
		Perquintill::from_percent(15),
		vec![].try_into().unwrap(),
	));
	assert_ok!(AcurastCompute::<T>::create_pool(
		RawOrigin::Root.into(),
		*b"v1_cpu_multi_core_______",
		Perquintill::from_percent(15),
		vec![].try_into().unwrap(),
	));
	assert_ok!(AcurastCompute::<T>::create_pool(
		RawOrigin::Root.into(),
		*b"v1_ram_total____________",
		Perquintill::from_percent(15),
		vec![].try_into().unwrap(),
	));
	assert_ok!(AcurastCompute::<T>::create_pool(
		RawOrigin::Root.into(),
		*b"v1_ram_speed____________",
		Perquintill::from_percent(15),
		vec![].try_into().unwrap(),
	));
	assert_ok!(AcurastCompute::<T>::create_pool(
		RawOrigin::Root.into(),
		*b"v1_storage_avail________",
		Perquintill::from_percent(15),
		vec![].try_into().unwrap(),
	));
	assert_ok!(AcurastCompute::<T>::create_pool(
		RawOrigin::Root.into(),
		*b"v1_storage_speed________",
		Perquintill::from_percent(15),
		vec![].try_into().unwrap(),
	));
}

fn register_submit_helper<T: Config>(
	account_index: u32,
	slots: u8,
) -> (T::AccountId, JobRegistrationFor<T>, JobIdSequence)
where
	T: pallet_balances::Config + pallet_acurast_compute::Config,
{
	let (caller, job): (T::AccountId, JobRegistrationFor<T>) =
		register_helper::<T>(account_index, slots);

	let register_call = Acurast::<T>::register_with_min_metrics(
		RawOrigin::Signed(caller.clone().into()).into(),
		job.clone(),
		vec![(1, 1, 2), (2, 1, 2), (3, 1, 2), (4, 1, 2), (5, 1, 2), (6, 1, 2)]
			.try_into()
			.unwrap(),
	);
	assert_ok!(register_call);
	let job_id = Acurast::<T>::job_id_sequence();

	(caller, job, job_id)
}

fn acknowledge_match_helper<T: Config>(
	consumer: Option<T::AccountId>,
	processor: Option<T::AccountId>,
) -> Result<(T::AccountId, JobRegistrationFor<T>, JobId<T::AccountId>), DispatchError>
where
	T: pallet_balances::Config,
{
	let consumer: T::AccountId =
		consumer.unwrap_or(<T as Config>::BenchmarkHelper::funded_account(0, u32::MAX.into()));
	let processor: T::AccountId =
		processor.unwrap_or(<T as Config>::BenchmarkHelper::funded_account(1, u32::MAX.into()));
	let ad = advertisement::<T>(1, 1_000_000);
	assert_ok!(
		AcurastMarketplace::<T>::advertise(RawOrigin::Signed(processor.clone()).into(), ad,)
	);
	let job = job_registration_with_reward::<T>(
		script(),
		1,
		100,
		1_000_000,
		1000,
		1,
		1000,
		None,
		Some(PlannedExecution { source: processor.clone(), start_delay: 0 }),
	);
	assert_ok!(Acurast::<T>::register(RawOrigin::Signed(consumer.clone()).into(), job.clone()));
	let job_id: JobId<T::AccountId> =
		(MultiOrigin::Acurast(consumer), Acurast::<T>::job_id_sequence());
	let status = AcurastMarketplace::<T>::stored_job_status(&job_id.0, job_id.1);
	assert_eq!(status, Some(JobStatus::Matched));
	Ok((processor, job, job_id))
}

fn acknowledge_execution_match_helper<T: Config>(
	consumer: Option<T::AccountId>,
	processor: Option<T::AccountId>,
) -> Result<(T::AccountId, JobRegistrationFor<T>, JobId<T::AccountId>), DispatchError>
where
	T: pallet_balances::Config + pallet_timestamp::Config,
	<T as pallet_timestamp::Config>::Moment: From<u64>,
{
	let consumer: T::AccountId =
		consumer.unwrap_or(<T as Config>::BenchmarkHelper::funded_account(0, u32::MAX.into()));
	let processor: T::AccountId =
		processor.unwrap_or(<T as Config>::BenchmarkHelper::funded_account(1, u32::MAX.into()));
	let ad = advertisement::<T>(1, 1_000_000);
	assert_ok!(
		AcurastMarketplace::<T>::advertise(RawOrigin::Signed(processor.clone()).into(), ad,)
	);
	let job = competing_job_registration_with_reward::<T>(script(), 1, 100, 1_000_000);

	pallet_timestamp::Pallet::<T>::set_timestamp((job.schedule.start_time - 310_000).into());

	assert_ok!(Acurast::<T>::register(RawOrigin::Signed(consumer.clone()).into(), job.clone()));
	let job_id: JobId<T::AccountId> =
		(MultiOrigin::Acurast(consumer.clone()), Acurast::<T>::job_id_sequence());

	pallet_timestamp::Pallet::<T>::set_timestamp((job.schedule.start_time - 120_000).into());

	assert_ok!(AcurastMarketplace::<T>::propose_execution_matching(
		RawOrigin::Signed(consumer.clone()).into(),
		vec![ExecutionMatch {
			job_id: job_id.clone(),
			execution_index: 0,
			sources: vec![PlannedExecution { source: processor.clone(), start_delay: 0 }]
				.try_into()
				.unwrap()
		}]
		.try_into()
		.unwrap()
	));

	pallet_timestamp::Pallet::<T>::set_timestamp(
		(job.schedule.start_time + job.schedule.interval - 120_000).into(),
	);

	assert_ok!(AcurastMarketplace::<T>::propose_execution_matching(
		RawOrigin::Signed(consumer.clone()).into(),
		vec![ExecutionMatch {
			job_id: job_id.clone(),
			execution_index: 1,
			sources: vec![PlannedExecution { source: processor.clone(), start_delay: 0 }]
				.try_into()
				.unwrap()
		}]
		.try_into()
		.unwrap()
	));

	let status = AcurastMarketplace::<T>::stored_job_status(&job_id.0, job_id.1);

	assert_eq!(status, Some(JobStatus::Matched));
	Ok((processor, job, job_id))
}

fn cleanup_storage_helper<T: Config>(
	consumer: Option<T::AccountId>,
	target_matches: u8,
) -> Result<JobId<T::AccountId>, DispatchError>
where
	T: pallet_balances::Config + pallet_timestamp::Config + pallet_acurast_compute::Config,
	<T as pallet_timestamp::Config>::Moment: From<u64>,
{
	let max_slots = <T as pallet_acurast::Config>::MaxSlots::get() as u8;
	let consumer: T::AccountId =
		consumer.unwrap_or(<T as Config>::BenchmarkHelper::funded_account(0, u32::MAX.into()));

	let job = competing_job_registration_with_reward::<T>(script(), max_slots, 100, 1_000_000);

	pallet_timestamp::Pallet::<T>::set_timestamp((job.schedule.start_time - 310_000).into());

	assert_ok!(Acurast::<T>::register(RawOrigin::Signed(consumer.clone()).into(), job.clone()));

	let job_id: JobId<T::AccountId> =
		(MultiOrigin::Acurast(consumer.clone()), Acurast::<T>::job_id_sequence());

	let needed_matches = target_matches.saturating_div(max_slots) + 1;

	let mut processor_counter: u32 = 0;

	for i in 0..needed_matches {
		pallet_timestamp::Pallet::<T>::set_timestamp(
			(job.schedule.start_time + (job.schedule.interval * (i as u64)) - 120_000).into(),
		);

		let mut planned_executions: Vec<PlannedExecution<T::AccountId>> = vec![];
		for _ in 0..max_slots {
			let processor_index = processor_counter;
			processor_counter += 1;
			let (processor, _) = advertise_helper::<T>(processor_index, true);
			planned_executions.push(PlannedExecution { source: processor, start_delay: 0 });
		}

		assert_ok!(AcurastMarketplace::<T>::propose_execution_matching(
			RawOrigin::Signed(consumer.clone()).into(),
			vec![ExecutionMatch {
				job_id: job_id.clone(),
				execution_index: i.into(),
				sources: planned_executions.try_into().unwrap()
			}]
			.try_into()
			.unwrap()
		));
	}

	let status = AcurastMarketplace::<T>::stored_job_status(&job_id.0, job_id.1);

	assert_eq!(status, Some(JobStatus::Matched));

	<T as Config>::BenchmarkHelper::remove_job_registration(&job_id);

	Ok(job_id)
}

fn propose_execution_matching_helper<T: Config>(
	processor_counter: Option<u32>,
) -> (JobRegistrationFor<T>, JobId<T::AccountId>, u32)
where
	T: pallet_balances::Config + pallet_timestamp::Config + pallet_acurast_compute::Config,
	<T as pallet_timestamp::Config>::Moment: From<u64>,
{
	let max_slots = <T as pallet_acurast::Config>::MaxSlots::get() as u8;
	let consumer: T::AccountId = <T as Config>::BenchmarkHelper::funded_account(0, u32::MAX.into());
	let job = competing_job_registration_with_reward::<T>(script(), max_slots, 100, 1_000_000);

	pallet_timestamp::Pallet::<T>::set_timestamp((job.schedule.start_time - 310_000).into());

	assert_ok!(Acurast::<T>::register_with_min_metrics(
		RawOrigin::Signed(consumer.clone()).into(),
		job.clone(),
		vec![(1, 1, 2), (2, 1, 2), (3, 1, 2), (4, 1, 2), (5, 1, 2), (6, 1, 2)]
			.try_into()
			.unwrap(),
	));

	let job_id: JobId<T::AccountId> =
		(MultiOrigin::Acurast(consumer.clone()), Acurast::<T>::job_id_sequence());

	let mut processor_counter: u32 = processor_counter.unwrap_or(0);

	for i in 0..2u8 {
		pallet_timestamp::Pallet::<T>::set_timestamp(
			(job.schedule.start_time + (job.schedule.interval * (i as u64)) - 120_000).into(),
		);

		let mut planned_executions: Vec<PlannedExecution<T::AccountId>> = vec![];
		for _ in 0..max_slots {
			let processor_index = processor_counter;
			processor_counter += 1;
			let (processor, _) = advertise_helper::<T>(processor_index, true);
			planned_executions.push(PlannedExecution { source: processor, start_delay: 0 });
		}

		assert_ok!(AcurastMarketplace::<T>::propose_execution_matching(
			RawOrigin::Signed(consumer.clone()).into(),
			vec![ExecutionMatch {
				job_id: job_id.clone(),
				execution_index: i.into(),
				sources: planned_executions.try_into().unwrap()
			}]
			.try_into()
			.unwrap()
		));
	}

	(job, job_id, processor_counter)
}

fn set_timestamp<T: pallet_timestamp::Config<Moment = u64>>(timestamp: u64) {
	pallet_timestamp::Pallet::<T>::set_timestamp(timestamp.into());
}

fn acknowledge_match_submit_helper<T: Config>(
	consumer: Option<T::AccountId>,
	processor: Option<T::AccountId>,
) -> Result<(T::AccountId, JobRegistrationFor<T>, JobId<T::AccountId>), DispatchError>
where
	T: pallet_balances::Config,
{
	let (processor_id, job, job_id) = acknowledge_match_helper::<T>(consumer, processor)?;
	let pub_keys: PubKeys = vec![
		PubKey::SECP256r1([0u8; 33].to_vec().try_into().unwrap()),
		PubKey::SECP256k1([0u8; 33].to_vec().try_into().unwrap()),
	]
	.try_into()
	.unwrap();
	let call = AcurastMarketplace::<T>::acknowledge_match(
		RawOrigin::Signed(processor_id.clone().into()).into(),
		job_id.clone(),
		pub_keys,
	);
	assert_ok!(call);
	Ok((processor_id, job, job_id))
}

benchmarks! {
	where_clause {  where
		T: pallet_acurast::Config + pallet_balances::Config + pallet_timestamp::Config<Moment = u64> + pallet_acurast_processor_manager::Config + pallet_acurast_compute::Config,
		<T as frame_system::Config>::AccountId: IsType<<<<T as pallet_acurast_processor_manager::Config>::Proof as Verify>::Signer as IdentifyAccount>::AccountId>,
	}

	advertise {
		set_timestamp::<T>(1000);
		// just create the data, do not submit the actual call (we want to benchmark `advertise`)
		let (caller, ad) = advertise_helper::<T>(0, false);
	}: _(RawOrigin::Signed(caller.clone()), ad.clone())
	verify {
		assert_last_event::<T>(Event::AdvertisementStored(
			ad, caller
		).into());
	}

	delete_advertisement {
		set_timestamp::<T>(1000);
		// create the data and submit so we have an add in storage to delete when benchmarking `delete_advertisement`
		let (caller, _) = advertise_helper::<T>(0, true);
	}: _(RawOrigin::Signed(caller.clone()))
	verify {
		assert_last_event::<T>(Event::AdvertisementRemoved(
			caller
		).into());
	}

	report {
		set_timestamp::<T>(1000);
		let (processor, job, job_id) = acknowledge_match_submit_helper::<T>(None, None)?;
		let manager: T::AccountId = <T as Config>::BenchmarkHelper::funded_account(2, u32::MAX.into());
		let (manager_id, _) = pallet_acurast_processor_manager::Pallet::<T>::do_get_or_create_manager_id(&manager)?;
		pallet_acurast_processor_manager::Pallet::<T>::do_add_processor_manager_pairing(&processor, manager_id)?;
		pallet_timestamp::Pallet::<T>::set_timestamp((job.schedule.nth_start_time(0, job.schedule.execution_count() - 1).unwrap() + job.schedule.duration).into());
	}: _(RawOrigin::Signed(processor), job_id, ExecutionResult::Success(vec![0u8].try_into().unwrap()))

	propose_matching {
		let x in 1 .. T::MaxProposedMatches::get();
		set_timestamp::<T>(1000);
		let caller: T::AccountId = <T as Config>::BenchmarkHelper::funded_account(0, 1_000_000_000_000u64.into());
		whitelist_account!(caller);
		let mut registered_jobs: Vec<(T::AccountId, JobRegistrationFor<T>, JobIdSequence)> = vec![];
		let max_slots = <T as pallet_acurast::Config>::MaxSlots::get();
		setup_pools::<T>();
		for i in 0..x {
			registered_jobs.push(register_submit_helper::<T>(i, max_slots as u8));
		}

		let mut current_account_index: u32 = 1;

		let matches: Vec<MatchFor<T>> = registered_jobs.into_iter().map(|(account_id, _, job_id)| {
			let mut processor_ids: Vec<T::AccountId> = vec![];
			for i in 0..max_slots {
				let account_index: u32 = current_account_index;
				current_account_index = current_account_index + 1;
				let (account_id, _) = advertise_helper::<T>(account_index, true);
				(&mut processor_ids).push(account_id);
			}
			Match {
				job_id: (MultiOrigin::Acurast(account_id), job_id),
				sources: processor_ids.into_iter().map(|account_id| PlannedExecution {
					source: account_id,
					start_delay: 0
				}).collect::<Vec<_>>().try_into().unwrap()
			}
		}).collect::<Vec<_>>();
	}: _(RawOrigin::Signed(caller), matches.try_into().unwrap())

	propose_execution_matching {
		let x in 1 .. T::MaxProposedExecutionMatches::get();
		set_timestamp::<T>(1000);
		let caller: T::AccountId = <T as Config>::BenchmarkHelper::funded_account(0, 1_000_000_000_000u64.into());
		whitelist_account!(caller);
		let mut registered_jobs: Vec<(JobRegistrationFor<T>, JobId<T::AccountId>)> = vec![];
		let max_slots = <T as pallet_acurast::Config>::MaxSlots::get();
		let mut current_account_index: u32 = 0;
		setup_pools::<T>();
		for i in 0..x {
			let (job, job_id, index) = propose_execution_matching_helper::<T>(Some(current_account_index));
			registered_jobs.push((job, job_id));
			current_account_index = index;
		}

		let matches: Vec<ExecutionMatchFor<T>> = registered_jobs.into_iter().map(|(job, job_id)| {
			pallet_timestamp::Pallet::<T>::set_timestamp(
				(job.schedule.start_time + (job.schedule.interval * 2) - 120_000).into(),
			);
			let mut processor_ids: Vec<T::AccountId> = vec![];
			for i in 0..max_slots {
				let account_index: u32 = current_account_index;
				current_account_index = current_account_index + 1;
				let (account_id, _) = advertise_helper::<T>(account_index, true);
				processor_ids.push(account_id);
			}
			ExecutionMatch {
				job_id,
				execution_index: 2,
				sources: processor_ids.into_iter().map(|account_id| PlannedExecution {
					source: account_id,
					start_delay: 0
				}).collect::<Vec<_>>().try_into().unwrap()
			}
		}).collect::<Vec<_>>();
	}: _(RawOrigin::Signed(caller), matches.try_into().unwrap())

	acknowledge_match {
		set_timestamp::<T>(1000);
		let (processor, _, job_id) = acknowledge_match_helper::<T>(None, None)?;
		let pub_keys: PubKeys = vec![PubKey::SECP256r1([0u8; 33].to_vec().try_into().unwrap()), PubKey::SECP256k1([0u8; 33].to_vec().try_into().unwrap())].try_into().unwrap();
	}: _(RawOrigin::Signed(processor), job_id, pub_keys)

	acknowledge_execution_match {
		set_timestamp::<T>(1000);
		let (processor, _, job_id) = acknowledge_execution_match_helper::<T>(None, None)?;
		let pub_keys: PubKeys = vec![PubKey::SECP256r1([0u8; 33].to_vec().try_into().unwrap()), PubKey::SECP256k1([0u8; 33].to_vec().try_into().unwrap())].try_into().unwrap();
	}: _(RawOrigin::Signed(processor), job_id, 1u64, pub_keys)

	finalize_job {
		set_timestamp::<T>(1000);
		let (processor, job, job_id) = acknowledge_match_submit_helper::<T>(None, None)?;
		let manager: T::AccountId = <T as Config>::BenchmarkHelper::funded_account(2, u32::MAX.into());
		let (manager_id, _) = pallet_acurast_processor_manager::Pallet::<T>::do_get_or_create_manager_id(&manager)?;
		pallet_acurast_processor_manager::Pallet::<T>::do_add_processor_manager_pairing(&processor, manager_id)?;
		pallet_timestamp::Pallet::<T>::set_timestamp((job.schedule.end_time + 1).into());
	}: _(RawOrigin::Signed(processor), job_id)

	finalize_jobs {
		let x in 1 .. T::MaxFinalizeJobs::get();
		set_timestamp::<T>(1000);
		let consumer = <T as Config>::BenchmarkHelper::funded_account(0, u32::MAX.into());
		let manager: T::AccountId = <T as Config>::BenchmarkHelper::funded_account(1, u32::MAX.into());
		let (manager_id, _) = pallet_acurast_processor_manager::Pallet::<T>::do_get_or_create_manager_id(&manager)?;
		let mut job_ids: Vec<JobIdSequence> = vec![];
		for i in 0..x {
			let processor = <T as Config>::BenchmarkHelper::funded_account(i + 2, u32::MAX.into());
			let (processor, job, job_id) = acknowledge_match_submit_helper::<T>(Some(consumer.clone()), Some(processor.clone()))?;
			pallet_acurast_processor_manager::Pallet::<T>::do_add_processor_manager_pairing(&processor, manager_id)?;
			(&mut job_ids).push(job_id.1);
		}
		pallet_timestamp::Pallet::<T>::set_timestamp((1689418800000u64 + 1).into());
	}: _(RawOrigin::Signed(consumer), job_ids.try_into().unwrap())

	cleanup_storage {
		let x in 1..u8::MAX.into();
		set_timestamp::<T>(1000);
		let job_id = cleanup_storage_helper::<T>(None, x as u8)?;
	}: _(RawOrigin::Root, job_id, x as u8)

	cleanup_assignments {
		let x in 1 .. T::MaxJobCleanups::get();
		set_timestamp::<T>(1000);
		let consumer = <T as Config>::BenchmarkHelper::funded_account(0, u32::MAX.into());
		let processor = <T as Config>::BenchmarkHelper::funded_account(1, u32::MAX.into());
		let (manager_id, _) = pallet_acurast_processor_manager::Pallet::<T>::do_get_or_create_manager_id(&consumer)?;
		pallet_acurast_processor_manager::Pallet::<T>::do_add_processor_manager_pairing(&processor, manager_id)?;
		let ad = advertisement::<T>(1, 1_000_000);
		assert_ok!(
			AcurastMarketplace::<T>::advertise(RawOrigin::Signed(processor.clone()).into(), ad)
		);
		let mut last_job: Option<JobRegistrationFor<T>> = None;
		let mut job_ids: Vec<JobId<T::AccountId>> = vec![];
		for i in 0..x {
			let job = job_registration_with_reward::<T>(
				script(),
				1,
				1,
				1,
				0, 0, 0,
				Some(i as u64),
				Some(PlannedExecution { source: processor.clone(), start_delay: 0 }),
			);
			assert_ok!(Acurast::<T>::register(RawOrigin::Signed(consumer.clone()).into(), job.clone()));
			let job_id_sequence = Acurast::<T>::job_id_sequence();
			job_ids.push((MultiOrigin::Acurast(consumer.clone()), job_id_sequence));
			last_job = Some(job);
		}
		let job = last_job.unwrap();
		pallet_timestamp::Pallet::<T>::set_timestamp((job.schedule.end_time + 1).into());
	}: _(RawOrigin::Signed(processor), job_ids.try_into().unwrap())

	impl_benchmark_test_suite!(AcurastMarketplace, mock::ExtBuilder::default().build(), mock::Test);
}
