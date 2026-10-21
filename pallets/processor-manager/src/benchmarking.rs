//! Benchmarking setup for pallet-acurast-processor-manager

use crate::stub::{alice_account_id, generate_account};

use super::*;

use acurast_common::{
	AdvertisementHandler, AttestationChain, ListUpdateOperation, MetricInput, MultiOrigin, PoolId,
	Version, METRICS_MAX_LENGTH,
};
use frame_benchmarking::v2::*;
use frame_support::{
	sp_runtime::{
		traits::{IdentifyAccount, StaticLookup, Verify},
		AccountId32,
	},
	traits::{Get, IsType},
	BoundedVec,
};
use frame_system::{pallet_prelude::BlockNumberFor, Pallet as System, RawOrigin};
use sp_std::prelude::*;

pub trait BenchmarkHelper<T: Config> {
	fn dummy_proof() -> T::Proof;
	fn advertisement() -> T::Advertisement;
	fn funded_account(index: u32) -> T::AccountId;
	fn attest_account(account: &T::AccountId);
	fn create_compute_pool() -> PoolId;
	fn max_compute_pools() -> u32;
	fn setup_compute_settings();
	fn commit(manager: &T::AccountId);
	fn pair_manager_and_processor(manager: &T::AccountId, processor: &T::AccountId);
	fn on_initialize(block_number: BlockNumberFor<T>);
	/// The runtime's `pallet_acurast_compute::Config::WarmupPeriod`.
	fn warmup_period() -> BlockNumberFor<T>;
	/// The runtime's `pallet_acurast_compute::Config::Epoch`.
	fn epoch() -> BlockNumberFor<T>;
	/// Fills all storage that `T::OnProcessorUnpaired` removes for `processor`, so that removing a
	/// pairing measures the worst case of the cleanup hooks configured for the runtime.
	fn setup_unpaired_cleanup(processor: &T::AccountId);
	/// Sets up the worst case of attesting a processor: its chain takes over the RKP certificate of another
	/// attested processor. Returns the processor and its chain.
	fn setup_rkp_takeover() -> (T::AccountId, AttestationChain);
}

fn generate_pairing_update_remove<T: Config>(index: u32) -> ProcessorPairingUpdateFor<T>
where
	T::AccountId: From<AccountId32>,
{
	let processor_account_id = generate_account(index).into();
	let timestamp = 1657363915002u128;
	let signature = T::BenchmarkHelper::dummy_proof();
	ProcessorPairingUpdateFor::<T> {
		operation: ListUpdateOperation::Remove,
		item: ProcessorPairingFor::<T>::new_with_proof(processor_account_id, timestamp, signature),
	}
}

/// Creates a pairing between `manager` and `processor` directly through the pallet internals.
///
/// `update_processor_pairings` can only remove pairings, so benchmark setups that need an existing
/// pairing cannot go through an extrinsic (`onboard` is the only pairing call and it validates an
/// attestation chain for a fixed processor account).
fn pair<T: Config>(manager: &T::AccountId, processor: &T::AccountId) -> Result<(), BenchmarkError> {
	let (manager_id, _) = Pallet::<T>::do_get_or_create_manager_id(manager)?;
	Pallet::<T>::do_add_processor_manager_pairing(processor, manager_id)?;

	Ok(())
}

/// The largest allowed consumers list.
fn max_allowed_consumers<T: Config>(
) -> BoundedVec<MultiOrigin<T::AccountId>, T::MaxAllowedConsumers>
where
	T::AccountId: From<AccountId32>,
{
	(0..T::MaxAllowedConsumers::get())
		.map(|i| MultiOrigin::Acurast(generate_account(i).into()))
		.collect::<Vec<_>>()
		.try_into()
		.unwrap()
}

/// Pairs `processor` with `manager` and stores the largest allowed consumers list for it.
fn pair_with_max_advertisement<T: Config>(
	manager: &T::AccountId,
	processor: &T::AccountId,
) -> Result<(), BenchmarkError>
where
	T::AccountId: From<AccountId32>,
{
	pair::<T>(manager, processor)?;
	T::AdvertisementHandler::advertise_for(processor, T::BenchmarkHelper::advertisement())?;
	T::AdvertisementHandler::update_allowed_consumers(processor, max_allowed_consumers::<T>())?;

	Ok(())
}

pub fn roll_to_block<T: Config>(block_number: BlockNumberFor<T>)
where
	BlockNumberFor<T>: IsType<u32>,
{
	let current_block: u32 = System::<T>::block_number().into();
	let start: u32 = current_block + 1;
	let end: u32 = block_number.into();
	for block in start..=end {
		System::<T>::set_block_number(block.into());
		T::BenchmarkHelper::on_initialize(block.into());
	}
}

/// A block `epochs` epochs after the warmup started by a heartbeat at block 100 has passed.
///
/// Each epoch step lands in the next compute epoch whatever the runtime's `Epoch` is, so a commit
/// there sees the metrics of the previous step as the last completed epoch's.
fn after_warmup<T: Config>(epochs: u32) -> BlockNumberFor<T> {
	T::BenchmarkHelper::warmup_period()
		+ T::BenchmarkHelper::epoch() * epochs.into()
		+ 101u32.into()
}

pub fn set_timestamp<T: pallet_timestamp::Config<Moment = u64>>(timestamp: u64) {
	pallet_timestamp::Now::<T>::put(timestamp);
}

pub fn processor_pairing<T: Config>(manager: T::AccountId) -> ProcessorPairingFor<T> {
	let timestamp = 1657363915002u128;
	let signature = T::BenchmarkHelper::dummy_proof();
	ProcessorPairingFor::<T>::new_with_proof(manager, timestamp, signature)
}

fn metric_inputs<T: Config>(count: u32) -> Vec<MetricInput> {
	let max_pools = T::BenchmarkHelper::max_compute_pools().max(1);
	let pool_ids: Vec<PoolId> = (0..count.min(max_pools))
		.map(|_| T::BenchmarkHelper::create_compute_pool())
		.collect();
	(0..count)
		.map(|i| (pool_ids[(i % pool_ids.len() as u32) as usize], 10u128, 1u128))
		.collect()
}

#[benchmarks(
	where
		T: Config + pallet_timestamp::Config<Moment = u64>,
		T::AccountId: IsType<<<T::Proof as Verify>::Signer as IdentifyAccount>::AccountId>,
		T::AccountId: From<AccountId32> + From<[u8; 32]>,
		BalanceFor<T>: IsType<u128>,
		BlockNumberFor<T>: IsType<u32>,
		<<T as frame_system::Config>::Lookup as StaticLookup>::Source: From<<<T::Proof as Verify>::Signer as IdentifyAccount>::AccountId>,
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn update_processor_pairings(
		x: Linear<1, { T::MaxPairingUpdates::get() }>,
	) -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		let mut updates = Vec::<ProcessorPairingUpdateFor<T>>::new();
		let caller: T::AccountId = alice_account_id().into();
		whitelist_account!(caller);
		for i in 0..x {
			// the call only supports removals, so every pairing to remove has to exist upfront
			let update = generate_pairing_update_remove::<T>(i + 1);
			pair::<T>(&caller, &update.item.account)?;
			// every removal triggers `T::OnProcessorUnpaired`; give it the maximum to clean up
			T::BenchmarkHelper::setup_unpaired_cleanup(&update.item.account);
			updates.push(update);
		}

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), updates.try_into().unwrap());

		Ok(())
	}

	#[benchmark]
	fn recover_funds() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		let caller: T::AccountId = alice_account_id().into();
		whitelist_account!(caller);
		// The processor must hold a reducible balance, otherwise `recover_assets` short-circuits on
		// `usable_balance > 0` and the burn/mint the extrinsic exists to perform is never measured.
		let processor: T::AccountId = T::BenchmarkHelper::funded_account(1);
		pair::<T>(&caller, &processor)?;
		// Kept distinct from `caller`: the origin's account is whitelisted, so minting into it would
		// hide the destination write.
		let destination: T::AccountId = generate_account(2).into();

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), processor.into().into(), destination.into().into());

		Ok(())
	}

	#[benchmark]
	fn heartbeat() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		let manager: T::AccountId = alice_account_id().into();
		let processor: T::AccountId = generate_account(1).into();
		whitelist_account!(processor);
		pair::<T>(&manager, &processor)?;

		#[extrinsic_call]
		_(RawOrigin::Signed(processor));

		Ok(())
	}

	#[benchmark]
	fn advertise_for() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		let caller: T::AccountId = alice_account_id().into();
		whitelist_account!(caller);
		let processor: T::AccountId = generate_account(1).into();
		pair_with_max_advertisement::<T>(&caller, &processor)?;
		let ad = T::BenchmarkHelper::advertisement();

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), processor.into().into(), ad);

		Ok(())
	}

	#[benchmark]
	fn update_allowed_consumers() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		let caller: T::AccountId = alice_account_id().into();
		whitelist_account!(caller);
		let processor: T::AccountId = generate_account(1).into();
		pair_with_max_advertisement::<T>(&caller, &processor)?;

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), processor.into().into(), max_allowed_consumers::<T>());

		Ok(())
	}

	#[benchmark]
	fn heartbeat_with_version() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		T::BenchmarkHelper::setup_compute_settings();
		let caller: T::AccountId = alice_account_id().into();
		let manager: T::AccountId = T::BenchmarkHelper::funded_account(0);
		whitelist_account!(caller);
		T::BenchmarkHelper::attest_account(&caller);
		roll_to_block::<T>(100u32.into());
		pair::<T>(&manager, &caller)?;
		assert_ne!(Pallet::<T>::manager_id_for_processor(&caller), None);
		let version = Version { platform: 0, build_number: 1 };

		let values = metric_inputs::<T>(6);

		// commit initially (starting warmup)
		Pallet::<T>::heartbeat_with_metrics(
			RawOrigin::Signed(caller.clone()).into(),
			version,
			values.clone().try_into().unwrap(),
		)?;

		// make sure warmup passed
		roll_to_block::<T>(after_warmup::<T>(0));
		Pallet::<T>::heartbeat_with_metrics(
			RawOrigin::Signed(caller.clone()).into(),
			version,
			values.clone().try_into().unwrap(),
		)?;

		roll_to_block::<T>(after_warmup::<T>(1));
		T::BenchmarkHelper::commit(&manager);
		Pallet::<T>::heartbeat_with_metrics(
			RawOrigin::Signed(caller.clone()).into(),
			version,
			values.clone().try_into().unwrap(),
		)?;

		roll_to_block::<T>(after_warmup::<T>(2));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), version);

		Ok(())
	}

	#[benchmark]
	fn heartbeat_with_version_no_claim() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		T::BenchmarkHelper::setup_compute_settings();
		let caller: T::AccountId = alice_account_id().into();
		let manager: T::AccountId = T::BenchmarkHelper::funded_account(0);
		whitelist_account!(caller);
		T::BenchmarkHelper::attest_account(&caller);
		roll_to_block::<T>(100u32.into());
		pair::<T>(&manager, &caller)?;
		assert_ne!(Pallet::<T>::manager_id_for_processor(&caller), None);
		let version = Version { platform: 0, build_number: 1 };

		let values = metric_inputs::<T>(6);

		// commit initially (starting warmup)
		Pallet::<T>::heartbeat_with_metrics(
			RawOrigin::Signed(caller.clone()).into(),
			version,
			values.clone().try_into().unwrap(),
		)?;

		// make sure warmup passed
		roll_to_block::<T>(after_warmup::<T>(0));

		#[extrinsic_call]
		heartbeat_with_version(RawOrigin::Signed(caller), version);

		Ok(())
	}

	#[benchmark]
	fn heartbeat_with_version_metrics_claim() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		T::BenchmarkHelper::setup_compute_settings();
		let caller: T::AccountId = alice_account_id().into();
		let manager: T::AccountId = T::BenchmarkHelper::funded_account(0);
		whitelist_account!(caller);
		T::BenchmarkHelper::attest_account(&caller);
		roll_to_block::<T>(100u32.into());
		pair::<T>(&manager, &caller)?;
		assert_ne!(Pallet::<T>::manager_id_for_processor(&caller), None);
		let version = Version { platform: 0, build_number: 1 };

		let values = metric_inputs::<T>(6);

		// commit initially (starting warmup)
		Pallet::<T>::heartbeat_with_metrics(
			RawOrigin::Signed(caller.clone()).into(),
			version,
			values.clone().try_into().unwrap(),
		)?;

		// make sure warmup passed
		roll_to_block::<T>(after_warmup::<T>(0));
		Pallet::<T>::heartbeat_with_metrics(
			RawOrigin::Signed(caller.clone()).into(),
			version,
			values.clone().try_into().unwrap(),
		)?;

		roll_to_block::<T>(after_warmup::<T>(1));

		#[extrinsic_call]
		heartbeat_with_version(RawOrigin::Signed(caller), version);

		Ok(())
	}

	#[benchmark]
	fn heartbeat_with_metrics(x: Linear<1, METRICS_MAX_LENGTH>) -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		T::BenchmarkHelper::setup_compute_settings();
		let caller: T::AccountId = alice_account_id().into();
		let manager: T::AccountId = T::BenchmarkHelper::funded_account(0);
		whitelist_account!(caller);
		T::BenchmarkHelper::attest_account(&caller);
		roll_to_block::<T>(100u32.into());
		pair::<T>(&manager, &caller)?;
		assert_ne!(Pallet::<T>::manager_id_for_processor(&caller), None);
		let version = Version { platform: 0, build_number: 1 };
		let values = metric_inputs::<T>(x);

		// commit initially (starting warmup)
		Pallet::<T>::heartbeat_with_metrics(
			RawOrigin::Signed(caller.clone()).into(),
			version,
			values.clone().try_into().unwrap(),
		)?;

		// make sure warmup passed
		roll_to_block::<T>(after_warmup::<T>(0));
		Pallet::<T>::heartbeat_with_metrics(
			RawOrigin::Signed(caller.clone()).into(),
			version,
			values.clone().try_into().unwrap(),
		)?;

		roll_to_block::<T>(after_warmup::<T>(1));
		T::BenchmarkHelper::commit(&manager);
		Pallet::<T>::heartbeat_with_metrics(
			RawOrigin::Signed(caller.clone()).into(),
			version,
			values.clone().try_into().unwrap(),
		)?;

		roll_to_block::<T>(after_warmup::<T>(2));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), version, values.try_into().unwrap());

		Ok(())
	}

	#[benchmark]
	fn heartbeat_with_metrics_no_claim(
		x: Linear<1, METRICS_MAX_LENGTH>,
	) -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		T::BenchmarkHelper::setup_compute_settings();
		let caller: T::AccountId = alice_account_id().into();
		let manager: T::AccountId = T::BenchmarkHelper::funded_account(0);
		whitelist_account!(caller);
		T::BenchmarkHelper::attest_account(&caller);
		roll_to_block::<T>(100u32.into());
		pair::<T>(&manager, &caller)?;
		assert_ne!(Pallet::<T>::manager_id_for_processor(&caller), None);
		let version = Version { platform: 0, build_number: 1 };
		let values = metric_inputs::<T>(x);

		// commit initially (starting warmup)
		Pallet::<T>::heartbeat_with_metrics(
			RawOrigin::Signed(caller.clone()).into(),
			version,
			values.clone().try_into().unwrap(),
		)?;

		// make sure warmup passed
		roll_to_block::<T>(after_warmup::<T>(0));

		#[extrinsic_call]
		heartbeat_with_metrics(RawOrigin::Signed(caller), version, values.try_into().unwrap());

		Ok(())
	}

	#[benchmark]
	fn heartbeat_with_metrics_claim(
		x: Linear<1, METRICS_MAX_LENGTH>,
	) -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		T::BenchmarkHelper::setup_compute_settings();
		let caller: T::AccountId = alice_account_id().into();
		let manager: T::AccountId = T::BenchmarkHelper::funded_account(0);
		whitelist_account!(caller);
		T::BenchmarkHelper::attest_account(&caller);
		roll_to_block::<T>(100u32.into());
		pair::<T>(&manager, &caller)?;
		assert_ne!(Pallet::<T>::manager_id_for_processor(&caller), None);
		let version = Version { platform: 0, build_number: 1 };
		let values = metric_inputs::<T>(x);

		// commit initially (starting warmup)
		Pallet::<T>::heartbeat_with_metrics(
			RawOrigin::Signed(caller.clone()).into(),
			version,
			values.clone().try_into().unwrap(),
		)?;

		// make sure warmup passed
		roll_to_block::<T>(after_warmup::<T>(0));
		Pallet::<T>::heartbeat_with_metrics(
			RawOrigin::Signed(caller.clone()).into(),
			version,
			values.clone().try_into().unwrap(),
		)?;

		roll_to_block::<T>(after_warmup::<T>(1));

		#[extrinsic_call]
		heartbeat_with_metrics(RawOrigin::Signed(caller), version, values.try_into().unwrap());

		Ok(())
	}

	#[benchmark]
	fn update_binary_hash() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		let version = Version { platform: 0, build_number: 1 };
		let hash: BinaryHash = [1; 32].into();

		#[extrinsic_call]
		_(RawOrigin::Root, version, Some(hash));

		Ok(())
	}

	#[benchmark]
	fn update_api_version() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		let version = 1;

		#[extrinsic_call]
		_(RawOrigin::Root, version);

		Ok(())
	}

	#[benchmark]
	fn set_processor_update_info(
		x: Linear<1, { T::MaxProcessorsInSetUpdateInfo::get() }>,
	) -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		let caller: T::AccountId = alice_account_id().into();
		whitelist_account!(caller);
		let mut processors = Vec::<T::AccountId>::new();
		for i in 0..x {
			let processor: T::AccountId = generate_account(i + 1).into();
			pair::<T>(&caller, &processor)?;
			processors.push(processor);
		}
		let version = Version { platform: 0, build_number: 1 };
		let hash: BinaryHash = [1; 32].into();
		Pallet::<T>::update_binary_hash(RawOrigin::Root.into(), version, Some(hash))?;
		let binary_location: BinaryLocation = b"https://github.com/Acurast/acurast-processor-update/releases/download/processor-1.3.31/processor-1.3.31-devnet.apk".to_vec().try_into().unwrap();
		let update_info = UpdateInfo { version, binary_location };

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), update_info, processors.try_into().unwrap());

		Ok(())
	}

	#[benchmark]
	fn update_reward_distribution_settings() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		let settings =
			RewardDistributionSettings::<BalanceFor<T>, <T as frame_system::Config>::AccountId> {
				window_length: 300,
				tollerance: 25,
				min_heartbeats: 3,
				reward_per_distribution: 300_000_000_000u128.into(),
				distributor_account: alice_account_id().into(),
			};

		#[extrinsic_call]
		_(RawOrigin::Root, Some(settings));

		Ok(())
	}

	#[benchmark]
	fn update_min_processor_version_for_reward() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		let version = Version { platform: 0, build_number: 100 };

		#[extrinsic_call]
		_(RawOrigin::Root, version);

		Ok(())
	}

	#[benchmark]
	fn set_management_endpoint() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		let endpoint: Endpoint = b"https://my-management-endpoint.io".to_vec().try_into().unwrap();
		let caller: T::AccountId = alice_account_id().into();
		whitelist_account!(caller);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), Some(endpoint));

		Ok(())
	}

	#[benchmark]
	fn onboard() -> Result<(), BenchmarkError> {
		let (processor_account, attestation_chain) = T::BenchmarkHelper::setup_rkp_takeover();
		let item = processor_pairing::<T>(generate_account(0).into());

		#[extrinsic_call]
		_(RawOrigin::Signed(processor_account), item, false, attestation_chain);

		Ok(())
	}

	#[benchmark]
	fn update_onboarding_settings() -> Result<(), BenchmarkError> {
		set_timestamp::<T>(1000);
		let settings = OnboardingSettings::<BalanceFor<T>, <T as frame_system::Config>::AccountId> {
			funds: 100_000_000_000u128.into(),
			max_funds: 1_000_000_000_000u128.into(),
			funds_account: alice_account_id().into(),
		};

		#[extrinsic_call]
		_(RawOrigin::Root, Some(settings));

		Ok(())
	}

	//impl_benchmark_test_suite!(Pallet, mock::ExtBuilder.build(), mock::Test);
}
