use frame_benchmarking::{benchmarks, whitelist_account};
use frame_support::{
	assert_ok,
	pallet_prelude::One,
	sp_runtime::{
		traits::{IdentifyAccount, Verify},
		DispatchError, FixedU128, Perquintill,
	},
	traits::{Currency, IsType},
};
use frame_system::{pallet_prelude::BlockNumberFor, RawOrigin};
use sp_core::*;
use sp_std::prelude::*;

use crate::Config;
use pallet_acurast::{
	ComputeHooks, JobId, JobIdSequence, JobModules, JobRegistrationFor, Metrics, MultiOrigin,
	Pallet as Acurast, Schedule, Script,
};
use pallet_acurast_compute::Pallet as AcurastCompute;

pub use crate::stub::*;
use crate::Pallet as AcurastMarketplace;

use super::*;

type BalanceFor<T> = <<T as pallet_acurast_compute::Config>::Currency as Currency<
	<T as frame_system::Config>::AccountId,
>>::Balance;

pub trait BenchmarkHelper<T: Config> {
	/// Extends the job requirements, defined by benchmarking code in this pallet, with the containing struct RegistrationExtra.
	fn registration_extra(r: JobRequirementsFor<T>) -> <T as Config>::RegistrationExtra;
	fn funded_account(index: u32, amount: T::Balance) -> T::AccountId;
	fn remove_job_registration(job_id: &JobId<T::AccountId>);
}

pub fn assert_last_event<T: Config>(generic_event: <T as frame_system::Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event);
}

pub fn advertisement<T: Config>(
	fee_per_millisecond: u128,
	storage_capacity: u32,
) -> AdvertisementFor<T> {
	Advertisement {
		pricing: Pricing {
			fee_per_millisecond: fee_per_millisecond.into(),
			fee_per_storage_byte: 0u8.into(),
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

/// The `start_time` of the schedules built by [`job_registration_with_reward`] and
/// [`competing_job_registration_with_reward`].
const SCHEDULE_START_TIME: u64 = 1689332400000;

/// The `end_time` of those schedules (one day after [`SCHEDULE_START_TIME`]).
const SCHEDULE_END_TIME: u64 = SCHEDULE_START_TIME + DAY;

/// The timestamp benchmarks start from.
///
/// `register_hook` bounds `start_time` to `Config::MaxStartWindow` into the future, so the clock has
/// to sit close to [`SCHEDULE_START_TIME`] for registrations to succeed. Benchmarks that need a
/// later clock (e.g. to report or finalize) move it forward themselves.
const BENCH_NOW: u64 = SCHEDULE_START_TIME - 300_000;

#[allow(clippy::too_many_arguments)]
pub fn job_registration_with_reward<T: Config>(
	script: Script,
	slots: u8,
	duration: u64,
	reward_value: u128,
	memory: u32,
	network_requests: u32,
	storage: u32,
	schedule_shift: Option<u64>,
	instant_match_processor: Option<Vec<PlannedExecution<T::AccountId>>>,
) -> JobRegistrationFor<T> {
	let reward: <T as Config>::Balance = reward_value.into();
	let r = JobRequirements {
		slots,
		reward,
		min_reputation: Some(0),
		assignment_strategy: AssignmentStrategy::Single(
			instant_match_processor.map(|m| m.try_into().unwrap()),
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
			start_time: SCHEDULE_START_TIME + (DAY * schedule_shift),
			end_time: SCHEDULE_END_TIME + (DAY * schedule_shift),
			interval: 180000, // 30min
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
			start_time: SCHEDULE_START_TIME,
			end_time: SCHEDULE_END_TIME,
			interval: 1800000, // 30min
			max_start_delay: 5000,
		},
		memory: 1_000u32,
		network_requests: 1,
		storage: 1_000u32,
		required_modules: JobModules::default(),
		extra: r,
	}
}

fn pool_metrics<T: pallet_acurast_compute::Config>() -> Vec<acurast_common::MetricInput> {
	(1..=pallet_acurast_compute::Pallet::<T>::last_metric_pool_id())
		.map(|pool_id| (pool_id, 1, 2))
		.collect()
}

fn advertise_helper<T>(account_index: u32, submit: bool) -> (T::AccountId, AdvertisementFor<T>)
where
	T: Config + pallet_balances::Config + pallet_acurast_compute::Config,
	BalanceFor<T>: IsType<u128>,
{
	let caller: T::AccountId =
		<T as Config>::BenchmarkHelper::funded_account(account_index, u64::MAX.into());
	whitelist_account!(caller);

	let ad = advertisement::<T>(1, 100_000);

	if submit {
		let register_call = AcurastMarketplace::<T>::advertise(
			RawOrigin::Signed(caller.clone()).into(),
			ad.clone(),
		);
		assert_ok!(register_call);
		let _ =
			AcurastCompute::<T>::commit(&caller, &(caller.clone(), 1.into()), &pool_metrics::<T>());
	}

	(caller, ad)
}

fn register_helper<T>(account_index: u32, slots: u8) -> (T::AccountId, JobRegistrationFor<T>)
where
	T: Config + pallet_balances::Config,
{
	let caller: T::AccountId =
		<T as Config>::BenchmarkHelper::funded_account(account_index, u64::MAX.into());
	whitelist_account!(caller);

	let job = job_registration_with_reward::<T>(
		script(),
		slots,
		// duration must be >= `Config::MinDuration` for registration to succeed; `interval` in
		// `job_registration_with_reward` (180000) is comfortably above the configured minimum.
		<T as Config>::MinDuration::get(),
		// reward per execution must cover `price_for` which is proportional to `duration`; sized
		// with margin for the (now larger) minimum duration.
		500_000_000_000,
		1000,
		1,
		1000,
		None,
		None,
	);

	(caller, job)
}

fn setup_pools<T: pallet_acurast_compute::Config>()
where
	BlockNumberFor<T>: One,
	BalanceFor<T>: From<u128>,
{
	const POOL_NAMES: [pallet_acurast_compute::MetricPoolName; 6] = [
		*b"v1_cpu_single_core______",
		*b"v1_cpu_multi_core_______",
		*b"v1_ram_total____________",
		*b"v1_ram_speed____________",
		*b"v1_storage_avail________",
		*b"v1_storage_speed________",
	];
	let max_pools = <<T as pallet_acurast_compute::Config>::MaxPools as Get<u32>>::get() as usize;
	for name in POOL_NAMES.iter().take(max_pools) {
		if AcurastCompute::<T>::metric_pool_lookup(name).is_some() {
			continue;
		}
		assert_ok!(AcurastCompute::<T>::create_pool(
			RawOrigin::Root.into(),
			*name,
			Perquintill::from_percent(15),
			vec![].try_into().unwrap(),
		));
	}
}

fn register_submit_helper<T>(
	account_index: u32,
	slots: u8,
) -> (T::AccountId, JobRegistrationFor<T>, JobId<T::AccountId>)
where
	T: Config + pallet_balances::Config + pallet_acurast_compute::Config,
{
	let (caller, job): (T::AccountId, JobRegistrationFor<T>) =
		register_helper::<T>(account_index, slots);

	let register_call = Acurast::<T>::register_with_min_metrics(
		RawOrigin::Signed(caller.clone()).into(),
		job.clone(),
		pool_metrics::<T>().try_into().unwrap(),
	);
	assert_ok!(register_call);
	let job_id_seq = Acurast::<T>::job_id_sequence();
	let job_id: JobId<T::AccountId> = (MultiOrigin::Acurast(caller.clone()), job_id_seq);

	(caller, job, job_id)
}

fn deploy_submit_helper<T>(
	account_index: u32,
	slots: u8,
) -> (T::AccountId, JobRegistrationFor<T>, JobId<T::AccountId>)
where
	T: Config + pallet_balances::Config,
{
	let (caller, job): (T::AccountId, JobRegistrationFor<T>) =
		register_helper::<T>(account_index, slots);

	assert_ok!(AcurastMarketplace::<T>::deploy(
		RawOrigin::Signed(caller.clone()).into(),
		job.clone(),
		pallet_acurast::ScriptMutability::Mutable(Some(caller.clone())),
		None,
		None
	));
	let job_id_seq = Acurast::<T>::job_id_sequence();
	let job_id = (MultiOrigin::Acurast(caller.clone()), job_id_seq);
	(caller, job, job_id)
}

#[allow(clippy::type_complexity)]
fn acknowledge_match_helper<T>(
	consumer: Option<T::AccountId>,
	processor: Option<T::AccountId>,
) -> Result<(T::AccountId, JobRegistrationFor<T>, JobId<T::AccountId>), DispatchError>
where
	T: Config + pallet_balances::Config,
{
	let consumer: T::AccountId =
		consumer.unwrap_or(<T as Config>::BenchmarkHelper::funded_account(0, u64::MAX.into()));
	let processor: T::AccountId =
		processor.unwrap_or(<T as Config>::BenchmarkHelper::funded_account(1, u64::MAX.into()));
	let ad = advertisement::<T>(1, 1_000_000);
	assert_ok!(
		AcurastMarketplace::<T>::advertise(RawOrigin::Signed(processor.clone()).into(), ad,)
	);
	let job = job_registration_with_reward::<T>(
		script(),
		1,
		<T as Config>::MinDuration::get(),
		500_000_000_000,
		1000,
		1,
		1000,
		None,
		Some(vec![PlannedExecution { source: processor.clone(), start_delay: 0 }]),
	);
	assert_ok!(Acurast::<T>::register(RawOrigin::Signed(consumer.clone()).into(), job.clone()));
	let job_id: JobId<T::AccountId> =
		(MultiOrigin::Acurast(consumer), Acurast::<T>::job_id_sequence());
	let status = AcurastMarketplace::<T>::stored_job_status(&job_id.0, job_id.1);
	assert_eq!(status, Some(JobStatus::Matched));
	Ok((processor, job, job_id))
}

#[allow(clippy::type_complexity)]
fn acknowledge_execution_match_helper<T>(
	consumer: Option<T::AccountId>,
	processor: Option<T::AccountId>,
) -> Result<(T::AccountId, JobRegistrationFor<T>, JobId<T::AccountId>), DispatchError>
where
	T: Config + pallet_balances::Config + pallet_timestamp::Config,
	<T as pallet_timestamp::Config>::Moment: From<u64>,
{
	let consumer: T::AccountId =
		consumer.unwrap_or(<T as Config>::BenchmarkHelper::funded_account(0, u64::MAX.into()));
	let processor: T::AccountId =
		processor.unwrap_or(<T as Config>::BenchmarkHelper::funded_account(1, u64::MAX.into()));
	let ad = advertisement::<T>(1, 1_000_000);
	assert_ok!(
		AcurastMarketplace::<T>::advertise(RawOrigin::Signed(processor.clone()).into(), ad,)
	);
	let job = competing_job_registration_with_reward::<T>(
		script(),
		1,
		<T as Config>::MinDuration::get(),
		500_000_000_000,
	);

	set_now::<T>(job.schedule.start_time - 310_000);

	assert_ok!(Acurast::<T>::register(RawOrigin::Signed(consumer.clone()).into(), job.clone()));
	let job_id: JobId<T::AccountId> =
		(MultiOrigin::Acurast(consumer.clone()), Acurast::<T>::job_id_sequence());

	set_now::<T>(job.schedule.start_time - 120_000);

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

	set_now::<T>(job.schedule.start_time + job.schedule.interval - 120_000);

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

fn cleanup_storage_helper<T>(
	consumer: Option<T::AccountId>,
	target_matches: u8,
) -> Result<JobId<T::AccountId>, DispatchError>
where
	T: Config + pallet_balances::Config + pallet_timestamp::Config + pallet_acurast_compute::Config,
	<T as pallet_timestamp::Config>::Moment: From<u64>,
	BalanceFor<T>: IsType<u128>,
{
	let max_slots = <T as pallet_acurast::Config>::MaxSlots::get() as u8;
	let consumer: T::AccountId =
		consumer.unwrap_or(<T as Config>::BenchmarkHelper::funded_account(0, u64::MAX.into()));

	let job = competing_job_registration_with_reward::<T>(
		script(),
		max_slots,
		<T as Config>::MinDuration::get(),
		500_000_000_000,
	);

	set_now::<T>(job.schedule.start_time - 310_000);

	assert_ok!(Acurast::<T>::register(RawOrigin::Signed(consumer.clone()).into(), job.clone()));

	let job_id: JobId<T::AccountId> =
		(MultiOrigin::Acurast(consumer.clone()), Acurast::<T>::job_id_sequence());

	let needed_matches = target_matches.saturating_div(max_slots) + 1;

	let mut processor_counter: u32 = 0;

	for i in 0..needed_matches {
		set_now::<T>(job.schedule.start_time + (job.schedule.interval * (i as u64)) - 120_000);

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

fn propose_execution_matching_helper<T>(
	processor_counter: Option<u32>,
) -> (JobRegistrationFor<T>, JobId<T::AccountId>, u32)
where
	T: Config + pallet_balances::Config + pallet_timestamp::Config + pallet_acurast_compute::Config,
	<T as pallet_timestamp::Config>::Moment: From<u64>,
	BalanceFor<T>: IsType<u128>,
{
	let max_slots = <T as pallet_acurast::Config>::MaxSlots::get() as u8;
	let consumer: T::AccountId = <T as Config>::BenchmarkHelper::funded_account(0, u64::MAX.into());
	let job = competing_job_registration_with_reward::<T>(
		script(),
		max_slots,
		<T as Config>::MinDuration::get(),
		500_000_000_000,
	);

	set_now::<T>(job.schedule.start_time - 310_000);

	assert_ok!(Acurast::<T>::register_with_min_metrics(
		RawOrigin::Signed(consumer.clone()).into(),
		job.clone(),
		pool_metrics::<T>().try_into().unwrap(),
	));

	let job_id: JobId<T::AccountId> =
		(MultiOrigin::Acurast(consumer.clone()), Acurast::<T>::job_id_sequence());

	let mut processor_counter: u32 = processor_counter.unwrap_or(0);

	for i in 0..2u8 {
		set_now::<T>(job.schedule.start_time + (job.schedule.interval * (i as u64)) - 120_000);

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
	pallet_timestamp::Now::<T>::put(timestamp);
}

fn set_now<T: pallet_timestamp::Config>(timestamp: u64)
where
	<T as pallet_timestamp::Config>::Moment: From<u64>,
{
	let now: <T as pallet_timestamp::Config>::Moment = timestamp.into();
	pallet_timestamp::Now::<T>::put(now);
}

#[allow(clippy::type_complexity)]
fn acknowledge_match_submit_helper<T>(
	consumer: Option<T::AccountId>,
	processor: Option<T::AccountId>,
) -> Result<(T::AccountId, JobRegistrationFor<T>, JobId<T::AccountId>), DispatchError>
where
	T: Config + pallet_balances::Config,
{
	let (processor_id, job, job_id) = acknowledge_match_helper::<T>(consumer, processor)?;
	let pub_keys: PubKeys = vec![
		PubKey::SECP256r1([0u8; 33].to_vec().try_into().unwrap()),
		PubKey::SECP256k1([0u8; 33].to_vec().try_into().unwrap()),
	]
	.try_into()
	.unwrap();
	let call = AcurastMarketplace::<T>::acknowledge_match(
		RawOrigin::Signed(processor_id.clone()).into(),
		job_id.clone(),
		pub_keys,
	);
	assert_ok!(call);
	Ok((processor_id, job, job_id))
}

/// Pre-populates `processor`'s `StoredMatches` with `count` already-acknowledged matches for
/// far-future (thus non-overlapping) jobs.
///
/// The matching extrinsics iterate over a processor's entire `StoredMatches` prefix in
/// `fits_schedule`; this iteration is bounded by [`Config::MaxMatchesPerProcessor`]. To benchmark
/// the worst case we fill each matched processor up to that bound so the measured weight reflects
/// the maximum schedule-fit iteration. The synthetic jobs are placed far in the future and marked
/// acknowledged so they are considered valid but never overlap the benchmarked job (which would
/// otherwise abort the matching call).
fn fill_processor_matches<T>(processor: &T::AccountId, count: u32)
where
	T: Config + pallet_balances::Config,
{
	let consumer: T::AccountId =
		<T as Config>::BenchmarkHelper::funded_account(u32::MAX, u64::MAX.into());
	for k in 0..count {
		// A large per-entry schedule shift (in days) keeps these jobs well beyond the benchmarked
		// job's schedule, so `fits_schedule` never reports an overlap for them.
		let job = job_registration_with_reward::<T>(
			script(),
			1,
			500,
			2_000_000_000,
			1000,
			1,
			1000,
			// clears even the worst-case ~9.76-year collider job used by `propose_matching`.
			Some(5000 + k as u64),
			None,
		);
		let job_id: JobId<T::AccountId> =
			(MultiOrigin::Acurast(consumer.clone()), 1_000_000u128 + k as u128);
		pallet_acurast::StoredJobRegistration::<T>::insert(&job_id.0, job_id.1, job);
		let assignment = Assignment {
			slot: 0,
			start_delay: 0,
			fee_per_execution: 0u64.into(),
			acknowledged: true,
			sla: SLA { total: 1, met: 0 },
			pub_keys: Default::default(),
			execution: ExecutionSpecifier::All,
		};
		crate::StoredMatches::<T>::insert(processor, &job_id, assignment);
	}
}

benchmarks! {
	where_clause {  where
		T: pallet_acurast::Config + pallet_balances::Config + pallet_timestamp::Config<Moment = u64> + pallet_acurast_processor_manager::Config + pallet_acurast_compute::Config,
		<T as frame_system::Config>::AccountId: IsType<<<<T as pallet_acurast_processor_manager::Config>::Proof as Verify>::Signer as IdentifyAccount>::AccountId>,
		BalanceFor<T>: IsType<u128>,
		BlockNumberFor<T>: One,
	}

	advertise {
		set_timestamp::<T>(BENCH_NOW);
		// just create the data, do not submit the actual call (we want to benchmark `advertise`)
		let (caller, ad) = advertise_helper::<T>(0, false);
	}: _(RawOrigin::Signed(caller.clone()), ad.clone())
	verify {
		assert_last_event::<T>(Event::AdvertisementStoredV2(
			caller
		).into());
	}

	delete_advertisement {
		set_timestamp::<T>(BENCH_NOW);
		// create the data and submit so we have an add in storage to delete when benchmarking `delete_advertisement`
		let (caller, _) = advertise_helper::<T>(0, true);
	}: _(RawOrigin::Signed(caller.clone()))
	verify {
		assert_last_event::<T>(Event::AdvertisementRemoved(
			caller
		).into());
	}

	report {
		set_timestamp::<T>(BENCH_NOW);
		let (processor, job, job_id) = acknowledge_match_submit_helper::<T>(None, None)?;
		let manager: T::AccountId = <T as Config>::BenchmarkHelper::funded_account(2, u64::MAX.into());
		let (manager_id, _) = pallet_acurast_processor_manager::Pallet::<T>::do_get_or_create_manager_id(&manager)?;
		pallet_acurast_processor_manager::Pallet::<T>::do_add_processor_manager_pairing(&processor, manager_id)?;
		pallet_timestamp::Now::<T>::put(job.schedule.nth_start_time(0, job.schedule.execution_count() - 1).unwrap() + job.schedule.duration);
	}: _(RawOrigin::Signed(processor), job_id, ExecutionResult::Success(vec![0u8].try_into().unwrap()))

	// Worst case: `x` matches, each filling every proposed processor to `MaxMatchesPerProcessor` so
	// `fits_schedule` performs its maximum (`MaxMatchesPerProcessor`-bounded) per-source iteration.
	// The processors carry only far-future (non-overlapping) fillers, so every `fits_schedule` short-
	// circuits on the O(1) `overlaps` prefilter and the call succeeds. The `(All, All)` per-execution
	// merge is not exercised on purpose: it is bounded by `MAX_EXECUTIONS_PER_JOB` and each step is a
	// trivial integer comparison, so it is negligible next to the per-source storage reads (and it can
	// only run to length on a call that ultimately errors, doing strictly less work than this path).
	propose_matching {
		let x in 1 .. T::MaxProposedMatches::get();
		set_timestamp::<T>(BENCH_NOW);
		let caller: T::AccountId = <T as Config>::BenchmarkHelper::funded_account(0, u64::MAX.into());
		whitelist_account!(caller);
		let max_slots = <T as pallet_acurast::Config>::MaxSlots::get();
		setup_pools::<T>();

		// The benchmarked call adds one match per processor, so fill up to the cap minus one to
		// exercise the worst-case per-processor schedule-fit iteration.
		let existing_matches = T::MaxMatchesPerProcessor::get().saturating_sub(1);

		let mut current_account_index: u32 = 1;
		let mut matches: Vec<MatchFor<T>> = vec![];

		for i in 0..x {
			let (_, _, job_id) = register_submit_helper::<T>(i, max_slots as u8);
			let mut processor_ids: Vec<T::AccountId> = vec![];
			for _ in 0..max_slots {
				let (account_id, _) = advertise_helper::<T>(current_account_index, true);
				current_account_index += 1;
				fill_processor_matches::<T>(&account_id, existing_matches);
				processor_ids.push(account_id);
			}
			matches.push(Match {
				job_id,
				sources: processor_ids.into_iter().map(|source| PlannedExecution {
					source,
					start_delay: 0,
				}).collect::<Vec<_>>().try_into().unwrap(),
			});
		}

	}: _(RawOrigin::Signed(caller), matches.try_into().unwrap())

	// Worst case fills every processor to `MaxMatchesPerProcessor`. Unlike `propose_matching`, the
	// matched job here is `Competing`, so each schedule-fit pair is `(Index, _)` and resolved
	// analytically in O(1) (`nth_start_time` + a single `overlaps`); the `(All, All)` per-execution
	// merge is never entered. `MAX_EXECUTIONS_PER_JOB` therefore does not affect this weight — only
	// the `MaxMatchesPerProcessor`-bounded outer loop does.
	propose_execution_matching {
		let x in 1 .. T::MaxProposedExecutionMatches::get();
		set_timestamp::<T>(BENCH_NOW);
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

		// The benchmarked call adds one match per processor, so fill up to the cap minus one to
		// exercise the worst-case per-processor schedule-fit iteration.
		let existing_matches = T::MaxMatchesPerProcessor::get().saturating_sub(1);
		let matches: Vec<ExecutionMatchFor<T>> = registered_jobs.into_iter().map(|(job, job_id)| {
			pallet_timestamp::Now::<T>::put(
				job.schedule.start_time + (job.schedule.interval * 2) - 120_000,
			);
			let mut processor_ids: Vec<T::AccountId> = vec![];
			for i in 0..max_slots {
				let account_index: u32 = current_account_index;
				current_account_index += 1;
				let (account_id, _) = advertise_helper::<T>(account_index, true);
				fill_processor_matches::<T>(&account_id, existing_matches);
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
		set_timestamp::<T>(BENCH_NOW);
		let (processor, _, job_id) = acknowledge_match_helper::<T>(None, None)?;
		let pub_keys: PubKeys = vec![PubKey::SECP256r1([0u8; 33].to_vec().try_into().unwrap()), PubKey::SECP256k1([0u8; 33].to_vec().try_into().unwrap())].try_into().unwrap();
	}: _(RawOrigin::Signed(processor), job_id, pub_keys)

	acknowledge_execution_match {
		set_timestamp::<T>(BENCH_NOW);
		let (processor, _, job_id) = acknowledge_execution_match_helper::<T>(None, None)?;
		let pub_keys: PubKeys = vec![PubKey::SECP256r1([0u8; 33].to_vec().try_into().unwrap()), PubKey::SECP256k1([0u8; 33].to_vec().try_into().unwrap())].try_into().unwrap();
	}: _(RawOrigin::Signed(processor), job_id, 1u64, pub_keys)

	finalize_job {
		set_timestamp::<T>(BENCH_NOW);
		let (processor, job, job_id) = acknowledge_match_submit_helper::<T>(None, None)?;
		let manager: T::AccountId = <T as Config>::BenchmarkHelper::funded_account(2, u64::MAX.into());
		let (manager_id, _) = pallet_acurast_processor_manager::Pallet::<T>::do_get_or_create_manager_id(&manager)?;
		pallet_acurast_processor_manager::Pallet::<T>::do_add_processor_manager_pairing(&processor, manager_id)?;
		pallet_timestamp::Now::<T>::put(job.schedule.end_time + 1);
	}: _(RawOrigin::Signed(processor), job_id)

	finalize_jobs {
		let x in 1 .. T::MaxFinalizeJobs::get();
		set_timestamp::<T>(BENCH_NOW);
		let consumer = <T as Config>::BenchmarkHelper::funded_account(0, u64::MAX.into());
		let manager: T::AccountId = <T as Config>::BenchmarkHelper::funded_account(1, u64::MAX.into());
		let (manager_id, _) = pallet_acurast_processor_manager::Pallet::<T>::do_get_or_create_manager_id(&manager)?;
		let mut job_ids: Vec<JobIdSequence> = vec![];
		for i in 0..x {
			let processor = <T as Config>::BenchmarkHelper::funded_account(i + 2, u64::MAX.into());
			let (processor, job, job_id) = acknowledge_match_submit_helper::<T>(Some(consumer.clone()), Some(processor.clone()))?;
			pallet_acurast_processor_manager::Pallet::<T>::do_add_processor_manager_pairing(&processor, manager_id)?;
			job_ids.push(job_id.1);
		}
		pallet_timestamp::Now::<T>::put(SCHEDULE_END_TIME + 1);
	}: _(RawOrigin::Signed(consumer), job_ids.try_into().unwrap())

	cleanup_storage {
		let x in 1..u8::MAX.into();
		set_timestamp::<T>(BENCH_NOW);
		let job_id = cleanup_storage_helper::<T>(None, x as u8)?;
	}: _(RawOrigin::Root, job_id, x as u8)

	cleanup_assignments {
		let x in 1 .. T::MaxJobCleanups::get().min(T::MaxMatchesPerProcessor::get());
		set_timestamp::<T>(BENCH_NOW);
		let consumer = <T as Config>::BenchmarkHelper::funded_account(0, u64::MAX.into());
		let processor = <T as Config>::BenchmarkHelper::funded_account(1, u64::MAX.into());
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
				<T as Config>::MinDuration::get(),
				500_000_000_000,
				0, 0, 0,
				Some(i as u64),
				Some(vec![PlannedExecution { source: processor.clone(), start_delay: 0 }]),
			);
			// each job is shifted one day further out to keep the per-processor schedules from
			// overlapping; advance the clock along with it so every `start_time` stays inside
			// `Config::MaxStartWindow` at the time of its registration
			pallet_timestamp::Now::<T>::put(job.schedule.start_time - 300_000);
			assert_ok!(Acurast::<T>::register(RawOrigin::Signed(consumer.clone()).into(), job.clone()));
			let job_id_sequence = Acurast::<T>::job_id_sequence();
			job_ids.push((MultiOrigin::Acurast(consumer.clone()), job_id_sequence));
			last_job = Some(job);
		}
		let job = last_job.unwrap();
		pallet_timestamp::Now::<T>::put(job.schedule.end_time + 1);
	}: _(RawOrigin::Signed(processor), job_ids.try_into().unwrap())

	// benchmark the worst case performance with mutable job that reuses keys
	deploy {
		set_timestamp::<T>(BENCH_NOW);
		let (_, _, original_job_id) = deploy_submit_helper::<T>(0, 1);

		let max_slots = <T as pallet_acurast::Config>::MaxSlots::get() as u8;
		let (caller, job): (T::AccountId, JobRegistrationFor<T>) =
			register_helper::<T>(0, max_slots);

		let job_id_seq = Acurast::<T>::job_id_sequence();
		let job_id: JobId<T::AccountId> = (MultiOrigin::Acurast(caller.clone()), job_id_seq);
		let min_metrics: Metrics = pool_metrics::<T>().try_into().unwrap();
	}: {
		assert_ok!(AcurastMarketplace::<T>::deploy(RawOrigin::Signed(caller.clone()).into(), job, pallet_acurast::ScriptMutability::Mutable(Some(caller)), Some(original_job_id), Some(min_metrics)));
	}

	edit_script {
		set_timestamp::<T>(BENCH_NOW);
		let (caller, _, job_id) = deploy_submit_helper::<T>(0, 1);
	}: {
		assert_ok!(AcurastMarketplace::<T>::edit_script(RawOrigin::Signed(caller.clone()).into(), job_id, script_random_value()));
	}

	transfer_editor {
		set_timestamp::<T>(BENCH_NOW);
		let (caller, _, job_id) = deploy_submit_helper::<T>(0, 1);

		let new_editor: T::AccountId =
		<T as Config>::BenchmarkHelper::funded_account(1, u64::MAX.into());
	}: {
		assert_ok!(AcurastMarketplace::<T>::transfer_editor(RawOrigin::Signed(caller.clone()).into(), job_id, Some(new_editor)));
	}

	update_min_fee_per_millisecond {
		let new_min_fee_per_millisecond: <T as Config>::Balance = 1000u128.into();
	}: {
		assert_ok!(AcurastMarketplace::<T>::update_min_fee_per_millisecond(RawOrigin::Root.into(), new_min_fee_per_millisecond));
	}

	cleanup_job_assignments {
		set_timestamp::<T>(BENCH_NOW);
		let slots: u8 = T::MaxSlots::get() as u8;
		let consumer = <T as Config>::BenchmarkHelper::funded_account(0, u64::MAX.into());
		let processors = (0..slots).map(|index| <T as Config>::BenchmarkHelper::funded_account((index + 1) as u32, 0u8.into())).collect::<Vec<_>>();
		let (manager_id, _) = pallet_acurast_processor_manager::Pallet::<T>::do_get_or_create_manager_id(&consumer)?;
		for processor in &processors {
			pallet_acurast_processor_manager::Pallet::<T>::do_add_processor_manager_pairing(processor, manager_id)?;
			let ad = advertisement::<T>(1, 1_000_000);
			assert_ok!(
				AcurastMarketplace::<T>::advertise(RawOrigin::Signed(processor.clone()).into(), ad)
			);
		}

		let job = job_registration_with_reward::<T>(
			script(),
			slots,
			<T as Config>::MinDuration::get(),
			500_000_000_000,
			0, 0, 0,
			None,
			Some(processors.into_iter().map(|processor| PlannedExecution { source: processor, start_delay: 0 }).collect::<Vec<_>>()),
		);
		assert_ok!(Acurast::<T>::register(RawOrigin::Signed(consumer.clone()).into(), job.clone()));
		let job_id_sequence = Acurast::<T>::job_id_sequence();
		let job_id = (MultiOrigin::Acurast(consumer.clone()), job_id_sequence);

		// The instant match must have produced one assignment per slot; otherwise the extrinsic below
		// has nothing to drain and the weight describes an empty call.
		assert_eq!(<AssignedProcessors<T>>::iter_prefix(&job_id).count(), slots as usize);

		// The clock has to pass the schedule's real expiry, which `is_expired` computes as
		// `actual_end(actual_start(max_start_delay)) + ReportTolerance` — NOT `end_time`.
		//
		// This was `end_time + 1`, which stopped being past expiry once this benchmark switched from
		// `duration: 1` to `Config::MinDuration` (60s): `actual_end` then landed 4_999 ms (exactly
		// `max_start_delay`) beyond `end_time + 1`, so `is_expired` was false, the drain loop never
		// ran, and the generated weight claimed 0 writes for an extrinsic that removes up to
		// `MaxSlots` (64) assignments — 128 writes in the previous, working weight set.
		let expiry = job
			.schedule
			.actual_end(job.schedule.actual_start(job.schedule.max_start_delay))
			.saturating_add(<T as Config>::ReportTolerance::get());
		pallet_timestamp::Now::<T>::put(expiry + 1);
		let job_id_after = job_id.clone();
	}: _(RawOrigin::Signed(consumer), job_id)
	verify {
		// Proves the worst case was actually measured: every assignment is gone.
		assert_eq!(<AssignedProcessors<T>>::iter_prefix(&job_id_after).count(), 0);
	}

	update_price_settings {
		let price_settings = PriceSettingsFor::<T> { min_price: 1000u128.into(), multiplier: FixedU128::from_rational(11, 10) };
	}: {
		assert_ok!(AcurastMarketplace::<T>::update_price_settings(RawOrigin::Root.into(), Some(price_settings)));
	}

	cleanup_job_matcher {
		set_timestamp::<T>(BENCH_NOW);
		let consumer = <T as Config>::BenchmarkHelper::funded_account(0, u64::MAX.into());
		let job_id = (MultiOrigin::Acurast(consumer.clone()), 1);
		<JobMatcher<T>>::insert(&job_id, consumer.clone());
	}: _(RawOrigin::Signed(consumer), job_id)

	//impl_benchmark_test_suite!(AcurastMarketplace, mock::ExtBuilder::default().build(), mock::Test);
}
