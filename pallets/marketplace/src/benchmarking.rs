use frame_benchmarking::{benchmarks, whitelist_account};
use frame_support::sp_runtime::{
    traits::{IdentifyAccount, Verify},
    DispatchError,
};
use frame_support::{assert_ok, traits::IsType};
use frame_system::RawOrigin;
use sp_core::*;
use sp_std::prelude::*;

use crate::Config;
use pallet_acurast::{
    JobId, JobIdSequence, JobModules, JobRegistrationFor, MultiOrigin, Pallet as Acurast, Schedule,
    Script,
};

pub use crate::stub::*;
use crate::Pallet as AcurastMarketplace;

use super::*;

pub trait BenchmarkHelper<T: Config> {
    /// Extends the job requirements, defined by benchmarking code in this pallet, with the containing struct RegistrationExtra.
    fn registration_extra(r: JobRequirementsFor<T>) -> <T as Config>::RegistrationExtra;
    fn funded_account(index: u32, amount: T::Balance) -> T::AccountId;
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

pub fn job_registration_with_reward<T: Config>(
    script: Script,
    slots: u8,
    duration: u64,
    reward_value: u128,
    instant_match_processor: Option<PlannedExecution<T::AccountId>>,
) -> JobRegistrationFor<T> {
    let reward: <T as Config>::Balance = reward_value.into();
    let r = JobRequirements {
        slots,
        reward,
        min_reputation: Some(0),
        instant_match: instant_match_processor.map(|m| vec![m].try_into().unwrap()),
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
            interval: 180000,          // 30min
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
    T: pallet_balances::Config,
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

    let job = job_registration_with_reward::<T>(script(), slots, 500, 20100, None);

    (caller, job)
}

fn register_submit_helper<T: Config>(
    account_index: u32,
    slots: u8,
) -> (T::AccountId, JobRegistrationFor<T>, JobIdSequence)
where
    T: pallet_balances::Config,
{
    let (caller, job): (T::AccountId, JobRegistrationFor<T>) =
        register_helper::<T>(account_index, slots);

    let register_call =
        Acurast::<T>::register(RawOrigin::Signed(caller.clone().into()).into(), job.clone());
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
    let consumer: T::AccountId = consumer.unwrap_or(
        <T as Config>::BenchmarkHelper::funded_account(0, u32::MAX.into()),
    );
    let processor: T::AccountId = processor.unwrap_or(
        <T as Config>::BenchmarkHelper::funded_account(1, u32::MAX.into()),
    );
    let ad = advertisement::<T>(1, 1_000_000);
    assert_ok!(AcurastMarketplace::<T>::advertise(
        RawOrigin::Signed(processor.clone()).into(),
        ad,
    ));
    let job = job_registration_with_reward::<T>(
        script(),
        1,
        100,
        1_000_000,
        Some(PlannedExecution {
            source: processor.clone(),
            start_delay: 0,
        }),
    );
    assert_ok!(Acurast::<T>::register(
        RawOrigin::Signed(consumer.clone()).into(),
        job.clone()
    ));
    let job_id: JobId<T::AccountId> = (
        MultiOrigin::Acurast(consumer),
        Acurast::<T>::job_id_sequence(),
    );
    let status = AcurastMarketplace::<T>::stored_job_status(&job_id.0, job_id.1);
    assert!(status == Some(JobStatus::Matched));
    Ok((processor, job, job_id))
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
        T: pallet_acurast::Config + pallet_balances::Config + pallet_timestamp::Config<Moment = u64> + pallet_acurast_processor_manager::Config,
        <T as frame_system::Config>::AccountId: IsType<<<<T as pallet_acurast_processor_manager::Config>::Proof as Verify>::Signer as IdentifyAccount>::AccountId>,
    }

    advertise {
        // just create the data, do not submit the actual call (we want to benchmark `advertise`)
        let (caller, ad) = advertise_helper::<T>(0, false);
    }: _(RawOrigin::Signed(caller.clone()), ad.clone())
    verify {
        assert_last_event::<T>(Event::AdvertisementStored(
            ad, caller
        ).into());
    }

    delete_advertisement {
        // create the data and submit so we have an add in storage to delete when benchmarking `delete_advertisement`
        let (caller, _) = advertise_helper::<T>(0, true);
    }: _(RawOrigin::Signed(caller.clone()))
    verify {
        assert_last_event::<T>(Event::AdvertisementRemoved(
            caller
        ).into());
    }

    report {
        let (processor, job, job_id) = acknowledge_match_submit_helper::<T>(None, None)?;
        let manager: T::AccountId = <T as Config>::BenchmarkHelper::funded_account(2, u32::MAX.into());
        let (manager_id, _) = pallet_acurast_processor_manager::Pallet::<T>::do_get_or_create_manager_id(&manager)?;
        pallet_acurast_processor_manager::Pallet::<T>::do_add_processor_manager_pairing(&processor, manager_id)?;
        pallet_timestamp::Pallet::<T>::set_timestamp(job.schedule.start_time.into());
    }: _(RawOrigin::Signed(processor), job_id, ExecutionResult::Success(vec![0u8].try_into().unwrap()))

    propose_matching {
        let x in 1 .. T::MaxProposedMatches::get();
        let caller: T::AccountId = <T as Config>::BenchmarkHelper::funded_account(0, 1_000_000_000_000u64.into());
        whitelist_account!(caller);
        let mut registered_jobs: Vec<(T::AccountId, JobRegistrationFor<T>, JobIdSequence)> = vec![];
        let max_slots = <T as pallet_acurast::Config>::MaxSlots::get();
        for i in 0..x {
            (&mut registered_jobs).push(register_submit_helper::<T>(i, max_slots as u8));
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

    acknowledge_match {
        let (processor, _, job_id) = acknowledge_match_helper::<T>(None, None)?;
        let pub_keys: PubKeys = vec![PubKey::SECP256r1([0u8; 33].to_vec().try_into().unwrap()), PubKey::SECP256k1([0u8; 33].to_vec().try_into().unwrap())].try_into().unwrap();
    }: _(RawOrigin::Signed(processor), job_id, pub_keys)

    finalize_job {
        let (processor, job, job_id) = acknowledge_match_submit_helper::<T>(None, None)?;
        let manager: T::AccountId = <T as Config>::BenchmarkHelper::funded_account(2, u32::MAX.into());
        let (manager_id, _) = pallet_acurast_processor_manager::Pallet::<T>::do_get_or_create_manager_id(&manager)?;
        pallet_acurast_processor_manager::Pallet::<T>::do_add_processor_manager_pairing(&processor, manager_id)?;
        pallet_timestamp::Pallet::<T>::set_timestamp((job.schedule.end_time + 1).into());
    }: _(RawOrigin::Signed(processor), job_id)

    finalize_jobs {
        let x in 1 .. T::MaxFinalizeJobs::get();
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

    impl_benchmark_test_suite!(AcurastMarketplace, mock::ExtBuilder::default().build(), mock::Test);
}
