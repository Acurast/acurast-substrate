use frame_benchmarking::v2::*;
use frame_support::{
	dispatch::RawOrigin,
	traits::{fungible::Mutate, Get, Hooks, IsType},
};
use frame_system::{pallet_prelude::BlockNumberFor, Pallet as System};
use sp_runtime::{traits::One, AccountId32, FixedU128, Perbill, Perquintill};
use sp_std::prelude::*;

use acurast_common::{ListUpdateOperation, MetricInput, PoolId, Version};
use pallet_acurast_processor_manager::{
	generate_account, BenchmarkHelper, Config as ProcessorManagerConfig,
	Pallet as ProcessorManager, ProcessorPairingFor, ProcessorPairingUpdateFor,
};

use crate::{stub::UNIT, types::*, Call, Config, Pallet};

fn generate_pairing_update_add<
	T: Config<I> + pallet_acurast_processor_manager::Config,
	I: 'static,
>(
	index: u32,
) -> ProcessorPairingUpdateFor<T>
where
	T::AccountId: From<AccountId32>,
{
	let processor_account_id = generate_account(index).into();
	let timestamp = 1657363915002u128;
	// let message = [caller.encode(), timestamp.encode(), 1u128.encode()].concat();
	let signature = <T as pallet_acurast_processor_manager::Config>::BenchmarkHelper::dummy_proof();
	ProcessorPairingUpdateFor::<T> {
		operation: ListUpdateOperation::Add,
		item: ProcessorPairingFor::<T>::new_with_proof(processor_account_id, timestamp, signature),
	}
}

pub fn roll_to_block<T: Config<I>, I: 'static>(block_number: BlockNumberFor<T>)
where
	BlockNumberFor<T>: IsType<u32>,
	BalanceFor<T, I>: From<u128>,
{
	let current_block: u32 = System::<T>::block_number().into();
	let start: u32 = current_block + 1;
	let end: u32 = block_number.into();
	for block in start..=end {
		System::<T>::set_block_number(block.into());
		Pallet::<T, I>::on_initialize(block.into());
	}
}

fn set_timestamp<T: pallet_timestamp::Config>(timestamp: u32) {
	pallet_timestamp::Pallet::<T>::set_timestamp(timestamp.into());
}

fn mint_to<T: Config<I>, I: 'static>(who: &T::AccountId, amount: BalanceFor<T, I>)
where
	<T as Config<I>>::Currency: Mutate<T::AccountId>,
{
	let _ = <<T as Config<I>>::Currency as Mutate<T::AccountId>>::mint_into(who, amount);
}

fn create_compute_pool<T: Config<I>, I: 'static>() -> PoolId
where
	BlockNumberFor<T>: One,
	BalanceFor<T, I>: From<u128>,
{
	let c = "abcdefghijklmnopqrstuvwxyz".as_bytes();
	let mut name = *b"cpu-ops-per-second______";
	name[23] = c[Pallet::<T, I>::last_metric_pool_id() as usize];

	Pallet::<T, I>::create_pool(
		RawOrigin::Root.into(),
		name,
		Perquintill::from_percent(25),
		Default::default(),
	)
	.expect("Expecting that pool creation always succeeds");
	Pallet::<T, I>::last_metric_pool_id()
}

#[instance_benchmarks(
	where
		T: Config<I> + pallet_timestamp::Config<Moment = u64> + pallet_acurast_processor_manager::Config,
		BlockNumberFor<T>: IsType<u32> + One,
		T::AccountId: From<AccountId32> + From<[u8; 32]>,
		<T as Config<I>>::Currency: Mutate<T::AccountId>,
		BalanceFor<T, I>: IsType<u128>,
		pallet_acurast_processor_manager::BalanceFor<T>: IsType<u128>,
		<T as frame_system::Config>::AccountId: frame_support::traits::IsType<<<<T as pallet_acurast_processor_manager::Config>::Proof as sp_runtime::traits::Verify>::Signer as sp_runtime::traits::IdentifyAccount>::AccountId>,
)]
mod benches {
	use super::{Pallet as Compute, *};

	#[benchmark]
	fn create_pool(n: Linear<1, CONFIG_VALUES_MAX_LENGTH>) {
		set_timestamp::<T>(1000);
		roll_to_block::<T, I>(100u32.into());

		let mut config_values = Vec::<MetricPoolConfigValue>::new();
		let c = "abcdefghijklmnopqrstuvwxyz".as_bytes();
		for i in 0..n {
			let mut config_name = *b"iterations______________";
			config_name[23] = c[i as usize];
			config_values.push((config_name, i.into(), i.into()));
		}

		#[extrinsic_call]
		_(
			RawOrigin::Root,
			*b"cpu-ops-per-second______",
			Perquintill::from_percent(20),
			config_values.try_into().unwrap(),
		);
	}

	#[benchmark]
	fn modify_pool_same_config() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		roll_to_block::<T, I>(100u32.into());

		let c = "abcdefghijklmnopqrstuvwxyz".as_bytes();
		let mut config_values = Vec::<MetricPoolConfigValue>::new();
		for i in 0..CONFIG_VALUES_MAX_LENGTH {
			let mut config_name = *b"iterations______________";
			config_name[23] = c[i as usize];
			config_values.push((config_name, i.into(), i.into()));
		}

		Compute::<T, I>::create_pool(
			RawOrigin::Root.into(),
			*b"cpu-ops-per-second______",
			Perquintill::from_percent(20),
			config_values.try_into().unwrap(),
		)?;

		#[extrinsic_call]
		Compute::<T, I>::modify_pool(
			RawOrigin::Root,
			1u8,
			Some(*b"cpu-ops-per-second-v2___"),
			Some((2u32.into(), Perquintill::from_percent(30))),
			None,
		);

		Ok(())
	}

	#[benchmark]
	fn modify_pool_replace_config(
		n: Linear<1, CONFIG_VALUES_MAX_LENGTH>,
	) -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		roll_to_block::<T, I>(100u32.into());

		let c = "abcdefghijklmnopqrstuvwxyz".as_bytes();
		let mut config_values = Vec::<MetricPoolConfigValue>::new();
		for i in 0..CONFIG_VALUES_MAX_LENGTH {
			let mut config_name = *b"iterations______________";
			config_name[23] = c[i as usize];
			config_values.push((config_name, i.into(), i.into()));
		}

		Compute::<T, I>::create_pool(
			RawOrigin::Root.into(),
			*b"cpu-ops-per-second______",
			Perquintill::from_percent(20),
			config_values.try_into().unwrap(),
		)?;

		let mut config_values = Vec::<MetricPoolConfigValue>::new();
		for i in 0..n {
			let mut config_name = *b"iterations______________";
			config_name[23] = c[i as usize];
			config_values.push((config_name, i.into(), i.into()));
		}
		let new_config = ModifyMetricPoolConfig::Replace(config_values.try_into().unwrap());

		#[extrinsic_call]
		Compute::<T, I>::modify_pool(
			RawOrigin::Root,
			1u8,
			Some(*b"cpu-ops-per-second-v2___"),
			Some((2u32.into(), Perquintill::from_percent(30))),
			Some(new_config),
		);

		Ok(())
	}

	#[benchmark]
	fn modify_pool_update_config(
		n: Linear<1, CONFIG_VALUES_MAX_LENGTH>,
	) -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		roll_to_block::<T, I>(100u32.into());

		let c = "abcdefghijklmnopqrstuvwxyz".as_bytes();
		let mut config_values = Vec::<MetricPoolConfigValue>::new();
		for i in 0..CONFIG_VALUES_MAX_LENGTH {
			let mut config_name = *b"iterations______________";
			config_name[23] = c[i as usize];
			config_values.push((config_name, i.into(), i.into()));
		}

		Compute::<T, I>::create_pool(
			RawOrigin::Root.into(),
			*b"cpu-ops-per-second______",
			Perquintill::from_percent(20),
			config_values.try_into().unwrap(),
		)?;

		let mut config_values = Vec::<MetricPoolConfigValue>::new();
		let mut remove = Vec::<MetricPoolConfigName>::new();
		for i in 0..n {
			let mut config_name = *b"iterations______________";
			remove.push(config_name);
			config_name[23] = c[i as usize];
			config_values.push((config_name, i.into(), i.into()));
		}
		let new_config = ModifyMetricPoolConfig::Update(MetricPoolUpdateOperations {
			add: config_values.try_into().unwrap(),
			remove: remove.try_into().unwrap(),
		});

		#[extrinsic_call]
		Compute::<T, I>::modify_pool(
			RawOrigin::Root,
			1u8,
			Some(*b"cpu-ops-per-second-v2___"),
			Some((2u32.into(), Perquintill::from_percent(30))),
			Some(new_config),
		);

		Ok(())
	}

	#[benchmark]
	fn offer_backing() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		roll_to_block::<T, I>(100u32.into());

		let manager: T::AccountId = account("manager", 0, 0);
		let committer: T::AccountId = account("commiter", 1, 1);

		let update = generate_pairing_update_add::<T, I>(0);
		ProcessorManager::<T>::update_processor_pairings(
			RawOrigin::Signed(manager.clone()).into(),
			vec![update.clone()].try_into().unwrap(),
		)?;

		#[extrinsic_call]
		_(RawOrigin::Signed(committer), manager);

		Ok(())
	}

	#[benchmark]
	fn withdraw_backing_offer() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		roll_to_block::<T, I>(100u32.into());

		let manager: T::AccountId = account("manager", 0, 0);
		let committer: T::AccountId = account("commiter", 1, 1);

		whitelist_account!(manager);
		let update = generate_pairing_update_add::<T, I>(0);
		ProcessorManager::<T>::update_processor_pairings(
			RawOrigin::Signed(manager.clone()).into(),
			vec![update.clone()].try_into().unwrap(),
		)?;

		Compute::<T, I>::offer_backing(RawOrigin::Signed(committer.clone()).into(), manager)?;

		#[extrinsic_call]
		_(RawOrigin::Signed(committer));

		Ok(())
	}

	#[benchmark]
	fn accept_backing_offer() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		roll_to_block::<T, I>(100u32.into());

		let manager: T::AccountId = account("manager", 0, 0);
		let committer: T::AccountId = account("commiter", 1, 1);

		let update = generate_pairing_update_add::<T, I>(0);
		ProcessorManager::<T>::update_processor_pairings(
			RawOrigin::Signed(manager.clone()).into(),
			vec![update.clone()].try_into().unwrap(),
		)?;

		Compute::<T, I>::offer_backing(
			RawOrigin::Signed(committer.clone()).into(),
			manager.clone(),
		)?;

		#[extrinsic_call]
		_(RawOrigin::Signed(manager), committer);

		Ok(())
	}

	#[benchmark]
	fn commit_compute(n: Linear<1, CONFIG_VALUES_MAX_LENGTH>) -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		roll_to_block::<T, I>(100u32.into());
		let manager: T::AccountId = account("manager", 0, 0);
		let processor: T::AccountId = account("processor", 1, 1);
		mint_to::<T, I>(&manager, (100 * UNIT).into());

		<T as ProcessorManagerConfig>::BenchmarkHelper::attest_account(&processor);
		<T as ProcessorManagerConfig>::BenchmarkHelper::pair_manager_and_processor(
			&manager, &processor,
		);

		let mut metrics = Vec::<MetricInput>::new();
		for _ in 0..n {
			let pool_id = create_compute_pool::<T, I>();
			metrics.push((pool_id, 10u128, 1u128));
		}

		let version = Version { platform: 0, build_number: 1 };
		ProcessorManager::<T>::heartbeat_with_metrics(
			RawOrigin::Signed(processor.clone()).into(),
			version,
			metrics.clone().try_into().unwrap(),
		)?;

		roll_to_block::<T, I>(1901u32.into());
		ProcessorManager::<T>::heartbeat_with_metrics(
			RawOrigin::Signed(processor.clone()).into(),
			version,
			metrics.clone().try_into().unwrap(),
		)?;

		let pool_ids = (1..=Compute::<T, I>::last_metric_pool_id()).collect::<Vec<_>>();
		let commitments = pool_ids
			.into_iter()
			.map(|pool_id| ComputeCommitment { pool_id, metric: FixedU128::from_rational(5, 1) })
			.collect::<Vec<_>>();

		roll_to_block::<T, I>(2701u32.into());

		Compute::<T, I>::offer_backing(RawOrigin::Signed(manager.clone()).into(), manager.clone())?;
		Compute::<T, I>::accept_backing_offer(
			RawOrigin::Signed(manager.clone()).into(),
			manager.clone(),
		)?;

		#[extrinsic_call]
		_(
			RawOrigin::Signed(manager),
			(50 * UNIT).into(),
			T::MinCooldownPeriod::get(),
			commitments.try_into().unwrap(),
			Perbill::from_percent(1),
			false,
		);

		Ok(())
	}
}
