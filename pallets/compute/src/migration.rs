use acurast_common::PoolId;
use frame_support::{
	traits::{
		// Currency,
		Get,
		GetStorageVersion,
		StorageVersion,
		// WithdrawReasons,
	},
	weights::Weight,
};
use sp_core::ConstU32;
use sp_runtime::{traits::Zero, BoundedVec};

use super::*;

pub fn migrate<T: Config<I>, I: 'static>() -> Weight {
	#[allow(clippy::type_complexity)]
	let migrations: [(u16, &dyn Fn() -> (Weight, bool)); 4] = [
		(6, &migrate_to_v6::<T, I>),
		(8, &migrate_to_v8::<T, I>),
		(9, &migrate_to_v9::<T, I>),
		(11, &migrate_to_v11::<T, I>),
	];

	let mut onchain_version = Pallet::<T, I>::on_chain_storage_version();
	let mut weight: Weight = Default::default();
	for (i, f) in migrations.into_iter() {
		let migrating_version = StorageVersion::new(i);
		if onchain_version < migrating_version {
			let (f_weight, completed) = f();
			weight += f_weight;
			if completed {
				migrating_version.put::<Pallet<T, I>>();
				onchain_version = migrating_version;
				weight = weight.saturating_add(T::DbWeight::get().writes(1));
			} else {
				break;
			}
		}
	}

	weight
}

/// Migrates `Cycle` from `Cycle<Epoch, Era, BlockNumber>` to `Cycle<Epoch, BlockNumber>` by removing era fields
/// and removes `max_stake_metric_ratio` from `MetricPool`
pub fn migrate_to_v6<T: Config<I>, I: 'static>() -> (Weight, bool) {
	let mut weight = Weight::zero();

	// Translate CurrentCycle storage
	let reads = if <CurrentCycle<T, I>>::exists() { 1 } else { 0 };
	weight = weight.saturating_add(T::DbWeight::get().reads(reads));

	let _ = <CurrentCycle<T, I>>::translate::<v5::CycleFor<T>, _>(|old_cycle_opt| {
		old_cycle_opt.map(|old_cycle| {
			// Keep only epoch and epoch_start, discard era and era_start
			CycleFor::<T> { epoch: old_cycle.epoch, epoch_start: old_cycle.epoch_start }
		})
	});

	weight = weight.saturating_add(T::DbWeight::get().writes(if reads > 0 { 1 } else { 0 }));

	// Translate MetricPools storage - remove max_stake_metric_ratio field
	weight = weight.saturating_add(
		T::DbWeight::get().reads(<MetricPools<T, I>>::iter_values().count() as u64),
	);
	<MetricPools<T, I>>::translate_values::<v5::MetricPoolFor<T>, _>(|old| {
		Some(MetricPoolFor::<T> {
			config: old.config,
			name: old.name,
			reward: old.reward,
			total: old.total,
			total_with_bonus: old.total,
		})
	});
	weight = weight.saturating_add(
		T::DbWeight::get().writes(<MetricPools<T, I>>::iter_values().count() as u64),
	);

	(weight, true)
}

/// Migrates `Scores` from `SlidingBuffer<BlockNumberFor<T>, U256>` to `SlidingBuffer<BlockNumberFor<T>, (U256, U256)>`
/// by converting each U256 score to (score, U256::zero()) tuple
pub fn migrate_to_v8<T: Config<I>, I: 'static>() -> (Weight, bool) {
	let mut weight = Weight::zero();

	// Count and translate all Scores entries
	let count = Scores::<T, I>::iter().count();
	weight = weight.saturating_add(T::DbWeight::get().reads(count as u64));

	Scores::<T, I>::translate::<SlidingBuffer<BlockNumberFor<T>, sp_core::U256>, _>(
		|_commitment_id, _pool_id, old_buffer| {
			// Convert old SlidingBuffer<BlockNumberFor<T>, U256> to new SlidingBuffer<BlockNumberFor<T>, (U256, U256)>
			// The old buffer stored just the score, the new one stores (score, bonus_score)
			// For migration, we set bonus_score to zero for all old entries
			Some(SlidingBuffer::new_with(old_buffer.epoch, (old_buffer.cur, sp_core::U256::zero())))
		},
	);

	weight = weight.saturating_add(T::DbWeight::get().writes(count as u64));

	(weight, true)
}

/// Migrates `Commitment` by adding the `last_slashing_epoch` field
pub fn migrate_to_v9<T: Config<I>, I: 'static>() -> (Weight, bool) {
	let mut weight = Weight::zero();

	// Count and translate all Commitment entries
	let count = Commitments::<T, I>::iter().count();
	weight = weight.saturating_add(T::DbWeight::get().reads(count as u64));

	Commitments::<T, I>::translate::<v8::CommitmentFor<T, I>, _>(
		|_commitment_id, old_commitment| {
			// Add the new last_slashing_epoch field, initialized to zero
			Some(CommitmentFor::<T, I> {
				stake: old_commitment.stake,
				commission: old_commitment.commission,
				delegations_total_amount: old_commitment.delegations_total_amount,
				delegations_total_rewardable_amount: old_commitment
					.delegations_total_rewardable_amount,
				weights: old_commitment.weights,
				pool_rewards: old_commitment.pool_rewards,
				last_scoring_epoch: old_commitment.last_scoring_epoch,
				last_slashing_epoch: Zero::zero(),
			})
		},
	);

	weight = weight.saturating_add(T::DbWeight::get().writes(count as u64));

	(weight, true)
}

pub fn migrate_to_v11<T: Config<I>, I: 'static>() -> (Weight, bool) {
	const CLEAR_LIMIT: u32 = 50;

	let mut migration_completed = false;
	let mut weight = T::DbWeight::get().reads(1);
	let cursor = V11MigrationState::<T, I>::get().map(|c| c.to_vec());
	if cursor.is_none() {
		crate::Pallet::<T, I>::deposit_event(Event::<T, I>::V11MigrationStarted);
	}
	let res = <MetricsEraAverage<T, I>>::clear(CLEAR_LIMIT, cursor.as_deref());
	weight = weight.saturating_add(T::DbWeight::get().writes(res.backend as u64));

	if let Some(new_cursor) = res.maybe_cursor {
		let bounded_cursor: Option<BoundedVec<u8, ConstU32<80>>> = new_cursor.try_into().ok();
		V11MigrationState::<T, I>::set(bounded_cursor);
	} else {
		migration_completed = true;
		V11MigrationState::<T, I>::kill();
		crate::Pallet::<T, I>::deposit_event(Event::<T, I>::V11MigrationCompleted);
	}
	weight = weight.saturating_add(T::DbWeight::get().writes(1));

	(weight, migration_completed)
}

pub mod v9 {
	use core::ops::Add;

	use super::*;
	use frame_support::pallet_prelude::*;
	use parity_scale_codec::{Decode, Encode};
	use sp_runtime::{
		traits::{Debug, One},
		FixedU128,
	};

	/// Old MetricPool struct without total_with_bonus field
	#[derive(
		RuntimeDebugNoBound, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq,
	)]
	pub struct MetricPool<
		Epoch: Copy + Ord + One + Add<Output = Epoch> + Debug,
		Value: Copy + Default + Debug,
	> {
		pub config: MetricPoolConfigValues,
		pub name: MetricPoolName,
		pub reward: ProvisionalBuffer<Epoch, Value>,
		pub total: SlidingBuffer<Epoch, FixedU128>,
	}
}

pub mod v8 {
	use core::ops::Add;

	use super::*;
	use frame_support::pallet_prelude::*;
	use parity_scale_codec::{Decode, Encode};
	use sp_runtime::{
		traits::{Debug, One},
		Perbill,
	};

	/// Old Commitment struct without last_slashing_epoch field
	#[derive(
		RuntimeDebugNoBound, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq,
	)]
	pub struct Commitment<
		Balance: Debug,
		BlockNumber: Debug + Ord + Copy,
		Epoch: Debug + Ord + Copy + One + Add<Output = Epoch>,
	> {
		pub stake: Option<Stake<Balance, BlockNumber>>,
		pub commission: Perbill,
		pub delegations_total_amount: Balance,
		pub delegations_total_rewardable_amount: Balance,
		pub weights: MemoryBuffer<Epoch, CommitmentWeights>,
		pub pool_rewards: MemoryBuffer<BlockNumber, PoolReward>,
		pub last_scoring_epoch: Epoch,
	}

	pub type CommitmentFor<T, I> = Commitment<BalanceFor<T, I>, BlockNumberFor<T>, EpochOf<T>>;
}

pub mod v5 {
	use core::ops::Add;

	use super::*;
	use frame_support::pallet_prelude::*;
	use parity_scale_codec::{Decode, Encode};
	use sp_runtime::{
		traits::{Debug, One},
		FixedU128, Perquintill,
	};

	#[derive(
		RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Copy, Clone, PartialEq, Eq, Default,
	)]
	pub struct Cycle<Epoch, Era, BlockNumber> {
		pub epoch: Epoch,
		pub epoch_start: BlockNumber,
		pub era: Era,
		pub era_start: BlockNumber,
	}

	pub type CycleFor<T> = Cycle<EpochOf<T>, EraOf<T>, BlockNumberFor<T>>;

	#[derive(
		RuntimeDebugNoBound, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq,
	)]
	pub struct MetricPool<
		Epoch: Copy + Ord + One + Add<Output = Epoch> + Debug,
		Value: Copy + Default + Debug,
	> {
		pub config: MetricPoolConfigValues,
		pub name: MetricPoolName,
		pub reward: ProvisionalBuffer<Epoch, Value>,
		pub total: SlidingBuffer<Epoch, FixedU128>,
		pub max_stake_metric_ratio: FixedU128,
	}

	pub type MetricPoolFor<T> = MetricPool<EpochOf<T>, Perquintill>;
}

pub mod v2 {
	use frame_support::pallet_prelude::*;
	use parity_scale_codec::{Decode, Encode, MaxEncodedLen};

	#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, MaxEncodedLen, TypeInfo)]
	pub struct RewardSettings<Balance, AccountId> {
		pub total_reward_per_distribution: Balance,
		pub distribution_account: AccountId,
	}
}

pub mod v1 {
	use core::ops::Add;

	use super::*;
	use frame_support::pallet_prelude::*;
	use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
	use sp_runtime::{
		traits::{Debug, One},
		FixedU128,
	};

	#[derive(
		RuntimeDebugNoBound, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq,
	)]
	pub struct MetricPool<
		Epoch: Copy + Ord + One + Add<Output = Epoch> + Debug,
		Value: Copy + Default + Debug,
	> {
		pub config: MetricPoolConfigValues,
		pub name: MetricPoolName,
		pub reward: ProvisionalBuffer<Epoch, Value>,
		pub total: SlidingBuffer<Epoch, FixedU128>,
	}
}
