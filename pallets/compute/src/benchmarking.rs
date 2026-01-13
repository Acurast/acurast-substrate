use frame_benchmarking::v2::*;
use frame_support::{
	dispatch::RawOrigin,
	traits::{fungible::Mutate, Get, Hooks, IsType},
};
use frame_system::{pallet_prelude::BlockNumberFor, Pallet as System};
use sp_runtime::{
	traits::{BlockNumberProvider, One},
	AccountId32, FixedU128, Perbill, Perquintill, Saturating,
};
use sp_std::prelude::*;

use acurast_common::{ListUpdateOperation, MetricInput, PoolId, Version};
use pallet_acurast_processor_manager::{
	generate_account, BenchmarkHelper, Config as ProcessorManagerConfig,
	Pallet as ProcessorManager, ProcessorPairingFor, ProcessorPairingUpdateFor,
};

use crate::{stub::UNIT, types::*, Call, Config, Pallet};

fn generate_pairing_update_add<T: Config<I> + ProcessorManagerConfig, I: 'static>(
	index: u32,
) -> ProcessorPairingUpdateFor<T>
where
	T::AccountId: From<AccountId32>,
{
	let processor_account_id = generate_account(index).into();
	let timestamp = 1657363915002u128;
	// let message = [caller.encode(), timestamp.encode(), 1u128.encode()].concat();
	let signature = <T as ProcessorManagerConfig>::BenchmarkHelper::dummy_proof();
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
	let mut name = *b"cpu-ops-per-second______";
	name[23] = Pallet::<T, I>::last_metric_pool_id();

	Pallet::<T, I>::create_pool(
		RawOrigin::Root.into(),
		name,
		Perquintill::from_percent(1),
		Default::default(),
	)
	.expect("Expecting that pool creation always succeeds");
	Pallet::<T, I>::last_metric_pool_id()
}

fn epoch_heartbeat<T: Config<I> + ProcessorManagerConfig, I: 'static>(
	processor: &T::AccountId,
) -> Result<(), BenchmarkError>
where
	BalanceFor<T, I>: IsType<u128>,
	pallet_acurast_processor_manager::BalanceFor<T>: IsType<u128>,
	BlockNumberFor<T>: IsType<u32> + One,
	<T as frame_system::Config>::AccountId: frame_support::traits::IsType<<<<T as pallet_acurast_processor_manager::Config>::Proof as sp_runtime::traits::Verify>::Signer as sp_runtime::traits::IdentifyAccount>::AccountId>,
{
	let mut metrics = Vec::<MetricInput>::new();
	let current_pools_count = Pallet::<T, I>::last_metric_pool_id() as u32;
	for index in 0..current_pools_count {
		metrics.push(((index + 1) as u8, 10u128, 1u128));
	}

	let version = Version { platform: 0, build_number: 1 };
	ProcessorManager::<T>::heartbeat_with_metrics(
		RawOrigin::Signed(processor.clone()).into(),
		version,
		metrics.clone().try_into().unwrap(),
	)?;

	let current_block = System::<T>::current_block_number();
	roll_to_block::<T, I>(current_block + T::Epoch::get());

	Ok(())
}

fn setup_stake<T: Config<I> + ProcessorManagerConfig, I: 'static>(
	manager: &T::AccountId,
	processor: &T::AccountId,
	commitments_count: u32,
	commit_compute: bool,
) -> Result<Vec<ComputeCommitment>, BenchmarkError> where
	<T as frame_system::Config>::AccountId: frame_support::traits::IsType<<<<T as pallet_acurast_processor_manager::Config>::Proof as sp_runtime::traits::Verify>::Signer as sp_runtime::traits::IdentifyAccount>::AccountId>,
	<T as Config<I>>::Currency: Mutate<T::AccountId>,
	BalanceFor<T, I>: IsType<u128>,
	BlockNumberFor<T>: IsType<u32> + One,
	pallet_acurast_processor_manager::BalanceFor<T>: IsType<u128>,
{
	let current_block = System::<T>::current_block_number();
	<T as ProcessorManagerConfig>::BenchmarkHelper::attest_account(processor);
	<T as ProcessorManagerConfig>::BenchmarkHelper::pair_manager_and_processor(manager, processor);

	let current_pools_count = Pallet::<T, I>::last_metric_pool_id() as u32;
	for _ in 0..commitments_count.saturating_sub(current_pools_count) {
		_ = create_compute_pool::<T, I>();
	}

	let mut metrics = Vec::<MetricInput>::new();
	let current_pools_count = Pallet::<T, I>::last_metric_pool_id() as u32;
	for index in 0..current_pools_count {
		metrics.push(((index + 1) as u8, 10u128, 1u128));
	}

	let version = Version { platform: 0, build_number: 1 };
	ProcessorManager::<T>::heartbeat_with_metrics(
		RawOrigin::Signed(processor.clone()).into(),
		version,
		metrics.clone().try_into().unwrap(),
	)?;

	roll_to_block::<T, I>(current_block + 1901u32.into());
	ProcessorManager::<T>::heartbeat_with_metrics(
		RawOrigin::Signed(processor.clone()).into(),
		version,
		metrics.clone().try_into().unwrap(),
	)?;

	let pool_ids = (1..=Pallet::<T, I>::last_metric_pool_id()).collect::<Vec<_>>();
	let commitments = pool_ids
		.into_iter()
		.map(|pool_id| ComputeCommitment { pool_id, metric: FixedU128::from_rational(5, 1) })
		.collect::<Vec<_>>();

	roll_to_block::<T, I>(current_block + 2701u32.into());

	Pallet::<T, I>::offer_backing(RawOrigin::Signed(manager.clone()).into(), manager.clone())?;
	Pallet::<T, I>::accept_backing_offer(
		RawOrigin::Signed(manager.clone()).into(),
		manager.clone(),
	)?;

	if commit_compute {
		Pallet::<T, I>::commit_compute(
			RawOrigin::Signed(manager.clone()).into(),
			T::MinStake::get(),
			T::MinCooldownPeriod::get(),
			commitments.clone().try_into().unwrap(),
			Perbill::from_percent(1),
			false,
		)?;
	}

	Ok(commitments)
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
	use sp_runtime::traits::BlockNumberProvider;

	use super::{Pallet as Compute, *};

	#[benchmark]
	fn create_pool(n: Linear<1, CONFIG_VALUES_MAX_LENGTH>) -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		Compute::<T, I>::enable_inflation(RawOrigin::Root.into())?;
		roll_to_block::<T, I>(100u32.into());

		let initial_pools_count = T::MaxPools::get().saturating_sub(1);
		for _ in 0..initial_pools_count {
			create_compute_pool::<T, I>();
		}

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

		Ok(())
	}

	#[benchmark]
	fn modify_pool_same_config() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		Compute::<T, I>::enable_inflation(RawOrigin::Root.into())?;
		roll_to_block::<T, I>(100u32.into());

		let initial_pools_count = T::MaxPools::get().saturating_sub(1);
		for _ in 0..initial_pools_count {
			create_compute_pool::<T, I>();
		}

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
		Compute::<T, I>::enable_inflation(RawOrigin::Root.into())?;
		roll_to_block::<T, I>(100u32.into());

		let initial_pools_count = T::MaxPools::get().saturating_sub(1);
		for _ in 0..initial_pools_count {
			create_compute_pool::<T, I>();
		}

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
		Compute::<T, I>::enable_inflation(RawOrigin::Root.into())?;
		roll_to_block::<T, I>(100u32.into());

		let initial_pools_count = T::MaxPools::get().saturating_sub(1);
		for _ in 0..initial_pools_count {
			create_compute_pool::<T, I>();
		}

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
		Compute::<T, I>::enable_inflation(RawOrigin::Root.into())?;
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
		Compute::<T, I>::enable_inflation(RawOrigin::Root.into())?;
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
		Compute::<T, I>::enable_inflation(RawOrigin::Root.into())?;
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
		Compute::<T, I>::enable_inflation(RawOrigin::Root.into())?;
		roll_to_block::<T, I>(100u32.into());
		let manager: T::AccountId = account("manager", 0, 0);
		let processor: T::AccountId = account("processor", 1, 1);
		mint_to::<T, I>(&manager, (100 * UNIT).into());

		let commitments = setup_stake::<T, I>(&manager, &processor, n, false)?;

		#[extrinsic_call]
		_(
			RawOrigin::Signed(manager),
			T::MinStake::get(),
			T::MinCooldownPeriod::get(),
			commitments.try_into().unwrap(),
			Perbill::from_percent(1),
			false,
		);

		Ok(())
	}

	#[benchmark]
	fn stake_more(n: Linear<1, CONFIG_VALUES_MAX_LENGTH>) -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		Compute::<T, I>::enable_inflation(RawOrigin::Root.into())?;
		roll_to_block::<T, I>(100u32.into());
		let manager: T::AccountId = account("manager", 0, 0);
		let processor: T::AccountId = account("processor", 1, 1);
		mint_to::<T, I>(&manager, (200 * UNIT).into());

		let commitments = setup_stake::<T, I>(&manager, &processor, n, true)?;

		#[extrinsic_call]
		_(
			RawOrigin::Signed(manager),
			T::MinStake::get(),
			Some(T::MinCooldownPeriod::get()),
			Some(commitments.try_into().unwrap()),
			Some(Perbill::from_percent(1)),
			Some(false),
		);

		Ok(())
	}

	#[benchmark]
	fn cooldown_compute_commitment() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		Compute::<T, I>::enable_inflation(RawOrigin::Root.into())?;
		roll_to_block::<T, I>(100u32.into());
		let manager: T::AccountId = account("manager", 0, 0);
		let processor: T::AccountId = account("processor", 1, 1);
		mint_to::<T, I>(&manager, (200 * UNIT).into());

		_ = setup_stake::<T, I>(&manager, &processor, CONFIG_VALUES_MAX_LENGTH, true)?;

		#[extrinsic_call]
		_(RawOrigin::Signed(manager));

		Ok(())
	}

	#[benchmark]
	fn end_compute_commitment() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		Compute::<T, I>::enable_inflation(RawOrigin::Root.into())?;
		roll_to_block::<T, I>(100u32.into());
		let manager: T::AccountId = account("manager", 0, 0);
		let processor: T::AccountId = account("processor", 1, 1);
		mint_to::<T, I>(&manager, (200 * UNIT).into());

		_ = setup_stake::<T, I>(&manager, &processor, CONFIG_VALUES_MAX_LENGTH, true)?;

		Compute::<T, I>::cooldown_compute_commitment(RawOrigin::Signed(manager.clone()).into())?;

		let current_block = System::<T>::current_block_number();

		roll_to_block::<T, I>(current_block + T::MinCooldownPeriod::get());

		#[extrinsic_call]
		_(RawOrigin::Signed(manager));

		Ok(())
	}

	#[benchmark]
	fn delegate() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		Compute::<T, I>::enable_inflation(RawOrigin::Root.into())?;
		roll_to_block::<T, I>(100u32.into());
		let manager: T::AccountId = account("manager", 0, 0);
		let processor: T::AccountId = account("processor", 1, 1);
		let delegator: T::AccountId = account("delegator", 2, 2);
		mint_to::<T, I>(&manager, (200 * UNIT).into());
		mint_to::<T, I>(&delegator, (100 * UNIT).into());

		_ = setup_stake::<T, I>(&manager, &processor, CONFIG_VALUES_MAX_LENGTH, true)?;

		#[extrinsic_call]
		_(
			RawOrigin::Signed(delegator),
			manager,
			T::MinDelegation::get(),
			T::MinCooldownPeriod::get(),
			false,
		);

		Ok(())
	}

	#[benchmark]
	fn cooldown_delegation() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		Compute::<T, I>::enable_inflation(RawOrigin::Root.into())?;
		roll_to_block::<T, I>(100u32.into());
		let manager: T::AccountId = account("manager", 0, 0);
		let processor: T::AccountId = account("processor", 1, 1);
		let delegator: T::AccountId = account("delegator", 2, 2);
		mint_to::<T, I>(&manager, (200 * UNIT).into());
		mint_to::<T, I>(&delegator, (100 * UNIT).into());

		_ = setup_stake::<T, I>(&manager, &processor, CONFIG_VALUES_MAX_LENGTH, true)?;

		Compute::<T, I>::delegate(
			RawOrigin::Signed(delegator.clone()).into(),
			manager.clone(),
			T::MinDelegation::get(),
			T::MinCooldownPeriod::get(),
			false,
		)?;

		#[extrinsic_call]
		_(RawOrigin::Signed(delegator.clone()), manager);

		Ok(())
	}

	#[benchmark]
	fn redelegate() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		Compute::<T, I>::enable_inflation(RawOrigin::Root.into())?;
		roll_to_block::<T, I>(100u32.into());
		let manager: T::AccountId = account("manager", 0, 0);
		let processor: T::AccountId = account("processor", 1, 1);
		let delegator: T::AccountId = account("delegator", 2, 2);
		mint_to::<T, I>(&manager, (200 * UNIT).into());
		mint_to::<T, I>(&delegator, (100 * UNIT).into());

		_ = setup_stake::<T, I>(&manager, &processor, CONFIG_VALUES_MAX_LENGTH, true)?;

		Compute::<T, I>::delegate(
			RawOrigin::Signed(delegator.clone()).into(),
			manager.clone(),
			T::MinDelegation::get(),
			T::MinCooldownPeriod::get(),
			false,
		)?;

		let manager_2: T::AccountId = account("manager", 3, 3);
		let processor_2: T::AccountId = account("processor", 4, 4);
		mint_to::<T, I>(&manager_2, (200 * UNIT).into());

		_ = setup_stake::<T, I>(&manager_2, &processor_2, CONFIG_VALUES_MAX_LENGTH, true)?;

		let current_block = System::<T>::current_block_number();

		roll_to_block::<T, I>(
			current_block + T::RedelegationBlockingPeriod::get().saturating_mul(T::Epoch::get()),
		);

		#[extrinsic_call]
		_(RawOrigin::Signed(delegator.clone()), manager, manager_2);

		Ok(())
	}

	#[benchmark]
	fn end_delegation() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		Compute::<T, I>::enable_inflation(RawOrigin::Root.into())?;
		roll_to_block::<T, I>(100u32.into());
		let manager: T::AccountId = account("manager", 0, 0);
		let processor: T::AccountId = account("processor", 1, 1);
		let delegator: T::AccountId = account("delegator", 2, 2);
		mint_to::<T, I>(&manager, (200 * UNIT).into());
		mint_to::<T, I>(&delegator, (100 * UNIT).into());

		_ = setup_stake::<T, I>(&manager, &processor, CONFIG_VALUES_MAX_LENGTH, true)?;

		Compute::<T, I>::delegate(
			RawOrigin::Signed(delegator.clone()).into(),
			manager.clone(),
			T::MinDelegation::get(),
			T::MinCooldownPeriod::get(),
			false,
		)?;

		Compute::<T, I>::cooldown_delegation(
			RawOrigin::Signed(delegator.clone()).into(),
			manager.clone(),
		)?;

		let current_block = System::<T>::current_block_number();
		roll_to_block::<T, I>(current_block + T::MinCooldownPeriod::get());

		#[extrinsic_call]
		_(RawOrigin::Signed(delegator.clone()), manager);

		Ok(())
	}

	#[benchmark]
	fn kick_out() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		Compute::<T, I>::enable_inflation(RawOrigin::Root.into())?;
		roll_to_block::<T, I>(100u32.into());
		let manager: T::AccountId = account("manager", 0, 0);
		let processor: T::AccountId = account("processor", 1, 1);
		let delegator: T::AccountId = account("delegator", 2, 2);
		mint_to::<T, I>(&manager, (200 * UNIT).into());
		mint_to::<T, I>(&delegator, (100 * UNIT).into());

		_ = setup_stake::<T, I>(&manager, &processor, CONFIG_VALUES_MAX_LENGTH, true)?;

		Compute::<T, I>::delegate(
			RawOrigin::Signed(delegator.clone()).into(),
			manager.clone(),
			T::MinDelegation::get(),
			T::MinCooldownPeriod::get(),
			false,
		)?;

		Compute::<T, I>::cooldown_compute_commitment(RawOrigin::Signed(manager.clone()).into())?;

		let current_block = System::<T>::current_block_number();
		roll_to_block::<T, I>(current_block + T::MinCooldownPeriod::get());

		Compute::<T, I>::end_compute_commitment(RawOrigin::Signed(manager.clone()).into())?;

		#[extrinsic_call]
		_(RawOrigin::Signed(manager.clone()), delegator, manager.clone());

		Ok(())
	}

	#[benchmark]
	fn slash() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		Compute::<T, I>::enable_inflation(RawOrigin::Root.into())?;
		roll_to_block::<T, I>(100u32.into());
		let manager: T::AccountId = account("manager", 0, 0);
		let processor: T::AccountId = account("processor", 1, 1);
		mint_to::<T, I>(&manager, (200 * UNIT).into());

		_ = setup_stake::<T, I>(&manager, &processor, CONFIG_VALUES_MAX_LENGTH, true)?;

		let current_block = System::<T>::current_block_number();
		roll_to_block::<T, I>(current_block + T::Epoch::get());

		#[extrinsic_call]
		_(RawOrigin::Signed(manager.clone()), manager.clone());

		Ok(())
	}

	#[benchmark]
	fn withdraw_delegation() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		Compute::<T, I>::enable_inflation(RawOrigin::Root.into())?;
		roll_to_block::<T, I>(100u32.into());
		let manager: T::AccountId = account("manager", 0, 0);
		let processor: T::AccountId = account("processor", 1, 1);
		let delegator: T::AccountId = account("delegator", 2, 2);
		mint_to::<T, I>(&manager, (200 * UNIT).into());
		mint_to::<T, I>(&delegator, (100 * UNIT).into());

		_ = setup_stake::<T, I>(&manager, &processor, CONFIG_VALUES_MAX_LENGTH, true)?;

		Compute::<T, I>::delegate(
			RawOrigin::Signed(delegator.clone()).into(),
			manager.clone(),
			T::MinDelegation::get(),
			T::MinCooldownPeriod::get(),
			false,
		)?;

		epoch_heartbeat::<T, I>(&processor)?;
		epoch_heartbeat::<T, I>(&processor)?;
		epoch_heartbeat::<T, I>(&processor)?;

		#[extrinsic_call]
		_(RawOrigin::Signed(delegator), manager);

		Ok(())
	}

	#[benchmark]
	fn withdraw_commitment() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		Compute::<T, I>::enable_inflation(RawOrigin::Root.into())?;
		roll_to_block::<T, I>(100u32.into());
		let manager: T::AccountId = account("manager", 0, 0);
		let processor: T::AccountId = account("processor", 1, 1);
		mint_to::<T, I>(&manager, (200 * UNIT).into());

		_ = setup_stake::<T, I>(&manager, &processor, CONFIG_VALUES_MAX_LENGTH, true)?;

		epoch_heartbeat::<T, I>(&processor)?;
		epoch_heartbeat::<T, I>(&processor)?;
		epoch_heartbeat::<T, I>(&processor)?;

		#[extrinsic_call]
		_(RawOrigin::Signed(manager));

		Ok(())
	}

	#[benchmark]
	fn delegate_more() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		Compute::<T, I>::enable_inflation(RawOrigin::Root.into())?;
		roll_to_block::<T, I>(100u32.into());
		let manager: T::AccountId = account("manager", 0, 0);
		let processor: T::AccountId = account("processor", 1, 1);
		let delegator: T::AccountId = account("delegator", 2, 2);
		mint_to::<T, I>(&manager, (200 * UNIT).into());
		mint_to::<T, I>(&delegator, (100 * UNIT).into());

		_ = setup_stake::<T, I>(&manager, &processor, CONFIG_VALUES_MAX_LENGTH, true)?;

		Compute::<T, I>::delegate(
			RawOrigin::Signed(delegator.clone()).into(),
			manager.clone(),
			T::MinDelegation::get(),
			T::MinCooldownPeriod::get(),
			false,
		)?;

		#[extrinsic_call]
		_(
			RawOrigin::Signed(delegator),
			manager,
			T::MinDelegation::get(),
			Some(T::MinCooldownPeriod::get()),
			Some(false),
		);

		Ok(())
	}

	#[benchmark]
	fn compound_delegation() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		Compute::<T, I>::enable_inflation(RawOrigin::Root.into())?;
		roll_to_block::<T, I>(100u32.into());
		let manager: T::AccountId = account("manager", 0, 0);
		let processor: T::AccountId = account("processor", 1, 1);
		let delegator: T::AccountId = account("delegator", 2, 2);
		mint_to::<T, I>(&manager, (200 * UNIT).into());
		mint_to::<T, I>(&delegator, (100 * UNIT).into());

		_ = setup_stake::<T, I>(&manager, &processor, CONFIG_VALUES_MAX_LENGTH, true)?;

		Compute::<T, I>::delegate(
			RawOrigin::Signed(delegator.clone()).into(),
			manager.clone(),
			T::MinDelegation::get(),
			T::MinCooldownPeriod::get(),
			false,
		)?;

		epoch_heartbeat::<T, I>(&processor)?;

		#[extrinsic_call]
		_(RawOrigin::Signed(delegator), manager, None);

		Ok(())
	}

	#[benchmark]
	fn compound_stake() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		Compute::<T, I>::enable_inflation(RawOrigin::Root.into())?;
		roll_to_block::<T, I>(100u32.into());
		let manager: T::AccountId = account("manager", 0, 0);
		let processor: T::AccountId = account("processor", 1, 1);
		mint_to::<T, I>(&manager, (200 * UNIT).into());

		_ = setup_stake::<T, I>(&manager, &processor, CONFIG_VALUES_MAX_LENGTH, true)?;

		epoch_heartbeat::<T, I>(&processor)?;

		#[extrinsic_call]
		_(RawOrigin::Signed(manager), None);

		Ok(())
	}

	#[benchmark]
	fn enable_inflation() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		roll_to_block::<T, I>(100u32.into());

		#[extrinsic_call]
		_(RawOrigin::Root);

		Ok(())
	}
}
