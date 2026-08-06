use frame_benchmarking::v2::*;
use frame_support::{
	dispatch::RawOrigin,
	traits::{
		fungible::{Inspect, Mutate, MutateHold},
		Get, Hooks, IsType,
	},
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
use pallet_acurast_token_conversion::Config as TokenConversionConfig;

use crate::{
	stub::{MILLIUNIT, UNIT},
	types::*,
	Call, Config, Pallet,
};

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

fn mint_to<T: Config<I> + TokenConversionConfig, I: 'static>(
	who: &T::AccountId,
	amount: BalanceFor<T, I>,
) where
	<T as Config<I>>::Currency: Mutate<T::AccountId>,
	BalanceFor<T, I>: IsType<u128>,
	<<T as TokenConversionConfig>::Currency as Inspect<T::AccountId>>::Balance: IsType<u128>,
{
	let hold_amount: u128 = amount.into();
	// leave a small free buffer above the held amount
	let mint_amount: u128 = hold_amount + MILLIUNIT;

	// The token-conversion processing path has been removed; establish the same
	// resulting state directly: fund the account, place a `HoldReason::Conversion`
	// hold on `amount` and record the lock (mirroring the former `process_conversion`).
	let _ = <<T as TokenConversionConfig>::Currency as Mutate<T::AccountId>>::mint_into(
		who,
		mint_amount.into(),
	);
	let _ = <<T as TokenConversionConfig>::Currency as MutateHold<T::AccountId>>::hold(
		&pallet_acurast_token_conversion::HoldReason::Conversion.into(),
		who,
		hold_amount.into(),
	);
	pallet_acurast_token_conversion::LockedConversion::<T>::insert(
		who,
		pallet_acurast_token_conversion::Conversion {
			amount: hold_amount.into(),
			lock_start: System::<T>::block_number(),
		},
	);
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

/// Rolls out the [`Config::RedelegationBlockingPeriod`] so a redelegation is allowed right away.
///
/// The period is anchored on the delegation's `stake.created`, which cannot simply be backdated: it
/// would have to stay `>= commitment.stake.created` to not count as stale, and the commitment is far
/// younger than the blocking period. So the blocks really have to be rolled — `16 * 900` on the kusama
/// config the weights are generated with.
fn expire_redelegation_blocking_period<T: Config<I>, I: 'static>()
where
	BlockNumberFor<T>: IsType<u32>,
	BalanceFor<T, I>: From<u128>,
{
	let current_block = System::<T>::current_block_number();
	roll_to_block::<T, I>(
		current_block + T::RedelegationBlockingPeriod::get().saturating_mul(T::Epoch::get()),
	);
}

/// Like [`setup_stake`], but for many manager/processor pairs at once.
///
/// [`setup_stake`] rolls ~2700 blocks *per call*, which makes setting up a dozen committers
/// prohibitively slow. Here the two heartbeat rounds are batched so the whole setup rolls ~2700 blocks
/// in total.
fn setup_stakes_many<T: Config<I> + ProcessorManagerConfig, I: 'static>(
	pairs: &[(T::AccountId, T::AccountId)],
	commitments_count: u32,
) -> Result<(), BenchmarkError> where
	<T as frame_system::Config>::AccountId: frame_support::traits::IsType<<<<T as pallet_acurast_processor_manager::Config>::Proof as sp_runtime::traits::Verify>::Signer as sp_runtime::traits::IdentifyAccount>::AccountId>,
	<T as Config<I>>::Currency: Mutate<T::AccountId>,
	BalanceFor<T, I>: IsType<u128>,
	BlockNumberFor<T>: IsType<u32> + One,
	pallet_acurast_processor_manager::BalanceFor<T>: IsType<u128>,
{
	let current_block = System::<T>::current_block_number();
	for (manager, processor) in pairs {
		<T as ProcessorManagerConfig>::BenchmarkHelper::attest_account(processor);
		<T as ProcessorManagerConfig>::BenchmarkHelper::pair_manager_and_processor(
			manager, processor,
		);
	}

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
	for (_, processor) in pairs {
		ProcessorManager::<T>::heartbeat_with_metrics(
			RawOrigin::Signed(processor.clone()).into(),
			version,
			metrics.clone().try_into().unwrap(),
		)?;
	}

	roll_to_block::<T, I>(current_block + 1901u32.into());
	for (_, processor) in pairs {
		ProcessorManager::<T>::heartbeat_with_metrics(
			RawOrigin::Signed(processor.clone()).into(),
			version,
			metrics.clone().try_into().unwrap(),
		)?;
	}

	let pool_ids = (1..=Pallet::<T, I>::last_metric_pool_id()).collect::<Vec<_>>();
	let commitments = pool_ids
		.into_iter()
		.map(|pool_id| ComputeCommitment { pool_id, metric: FixedU128::from_rational(5, 1) })
		.collect::<Vec<_>>();

	roll_to_block::<T, I>(current_block + 2701u32.into());

	for (manager, _) in pairs {
		Pallet::<T, I>::offer_backing(RawOrigin::Signed(manager.clone()).into(), manager.clone())?;
		Pallet::<T, I>::accept_backing_offer(
			RawOrigin::Signed(manager.clone()).into(),
			manager.clone(),
		)?;
		Pallet::<T, I>::commit_compute(
			RawOrigin::Signed(manager.clone()).into(),
			T::MinStake::get(),
			T::MinCooldownPeriod::get(),
			commitments.clone().try_into().unwrap(),
			Perbill::from_percent(1),
			false,
		)?;
	}

	Ok(())
}

#[instance_benchmarks(
	where
		T: Config<I> + pallet_timestamp::Config<Moment = u64> + ProcessorManagerConfig + TokenConversionConfig,
		BlockNumberFor<T>: IsType<u32> + One,
		T::AccountId: From<AccountId32> + From<[u8; 32]>,
		<T as Config<I>>::Currency: Mutate<T::AccountId>,
		BalanceFor<T, I>: IsType<u128>,
		pallet_acurast_processor_manager::BalanceFor<T>: IsType<u128>,
		<<T as TokenConversionConfig>::Currency as Inspect<T::AccountId>>::Balance: IsType<u128>,
		<T as frame_system::Config>::AccountId: IsType<<<<T as ProcessorManagerConfig>::Proof as sp_runtime::traits::Verify>::Signer as sp_runtime::traits::IdentifyAccount>::AccountId>,
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
	#[allow(deprecated)] // benchmarks the still-dispatchable, deprecated `redelegate` full move
	fn redelegate() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		Compute::<T, I>::enable_inflation(RawOrigin::Root.into())?;
		roll_to_block::<T, I>(100u32.into());
		let manager: T::AccountId = account("manager", 0, 0);
		let processor: T::AccountId = account("processor", 1, 1);
		let manager_2: T::AccountId = account("manager", 3, 3);
		let processor_2: T::AccountId = account("processor", 4, 4);
		let delegator: T::AccountId = account("delegator", 2, 2);
		mint_to::<T, I>(&manager, (200 * UNIT).into());
		mint_to::<T, I>(&manager_2, (200 * UNIT).into());
		mint_to::<T, I>(&delegator, (100 * UNIT).into());

		setup_stakes_many::<T, I>(
			&[(manager.clone(), processor), (manager_2.clone(), processor_2)],
			CONFIG_VALUES_MAX_LENGTH,
		)?;

		Compute::<T, I>::delegate(
			RawOrigin::Signed(delegator.clone()).into(),
			manager.clone(),
			T::MinDelegation::get(),
			T::MinCooldownPeriod::get(),
			false,
		)?;

		expire_redelegation_blocking_period::<T, I>();

		#[extrinsic_call]
		_(RawOrigin::Signed(delegator.clone()), manager, manager_2);

		Ok(())
	}

	#[benchmark]
	fn redelegate_v2(n: Linear<1, MAX_REDELEGATIONS>) -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		Compute::<T, I>::enable_inflation(RawOrigin::Root.into())?;
		roll_to_block::<T, I>(100u32.into());
		let manager: T::AccountId = account("manager", 0, 0);
		let processor: T::AccountId = account("processor", 1, 1);
		let delegator: T::AccountId = account("delegator", 2, 2);
		mint_to::<T, I>(&manager, (200 * UNIT).into());
		mint_to::<T, I>(&delegator, (100 * UNIT).into());

		let mut pairs = vec![(manager.clone(), processor)];
		let target_managers = (0..MAX_REDELEGATIONS)
			.map(|i| {
				let target_manager: T::AccountId = account("target_manager", i, i);
				let target_processor: T::AccountId = account("target_processor", i, i);
				mint_to::<T, I>(&target_manager, (200 * UNIT).into());
				pairs.push((target_manager.clone(), target_processor));
				target_manager
			})
			.collect::<Vec<_>>();

		setup_stakes_many::<T, I>(&pairs, CONFIG_VALUES_MAX_LENGTH)?;

		let min_delegation = T::MinDelegation::get();
		// No target is delegated to yet — a redelegation rejects a target the delegator already delegates
		// to, so all `n` legs are a fresh `delegate_for`, which is also what a leg keeping stake with the
		// old committer costs.
		//
		// Exactly one minimum delegation per target, since the targets must account for the whole
		// delegated amount.
		Compute::<T, I>::delegate(
			RawOrigin::Signed(delegator.clone()).into(),
			manager.clone(),
			min_delegation.saturating_mul((n as u128).into()),
			T::MinCooldownPeriod::get(),
			false,
		)?;

		expire_redelegation_blocking_period::<T, I>();

		let targets: RedelegationTargetsFor<T, I> = target_managers
			.into_iter()
			.take(n as usize)
			.map(|target_manager| (target_manager, min_delegation))
			.collect::<Vec<_>>()
			.try_into()
			.unwrap();

		#[extrinsic_call]
		_(RawOrigin::Signed(delegator.clone()), manager, targets);

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
