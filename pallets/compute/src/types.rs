use core::ops::Add;

use acurast_common::PoolId;
use frame_support::pallet_prelude::*;
use sp_runtime::{
	traits::{Debug, One, Saturating},
	FixedU128, Perbill, Perquintill,
};

use crate::{
	datastructures::{ProvisionalBuffer, SlidingBuffer},
	Config, EpochOf,
};

pub type MetricPoolFor<T, I> = MetricPool<EpochOf<T, I>, Perquintill>;
pub type ProcessorStateFor<T, I> = ProcessorState<
	<T as Config<I>>::BlockNumber,
	<T as Config<I>>::BlockNumber,
	<T as Config<I>>::Balance,
>;
pub type ProcessorStatusFor<T, I> = ProcessorStatus<<T as Config<I>>::BlockNumber>;
pub type MetricCommitFor<T, I> = MetricCommit<<T as Config<I>>::BlockNumber>;

pub const CONFIG_VALUES_MAX_LENGTH: u32 = 20;
pub type MetricPoolConfigValues =
	BoundedVec<MetricPoolConfigValue, ConstU32<CONFIG_VALUES_MAX_LENGTH>>;

pub type MetricPoolConfigValue = (MetricPoolConfigName, u128, u128);

/// The type of a metric.
pub type Metric = FixedU128;

/// The type of a metric pool's name.
pub type MetricPoolName = [u8; 24];

pub type MetricPoolConfigName = [u8; 24];

pub type StakeFor<T, I> = Stake<<T as Config<I>>::Balance, <T as Config<I>>::BlockNumber>;
pub type DelegateeTotalFor<T, I> = DelegateeTotal<<T as Config<I>>::Balance>;

#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	TypeInfo,
	RuntimeDebugNoBound,
	Clone,
	PartialEq,
	Eq,
)]
pub enum ModifyMetricPoolConfig {
	Replace(MetricPoolConfigValues),
	Update(MetricPoolUpdateOperations),
}

#[derive(
	RuntimeDebugNoBound,
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	TypeInfo,
	Clone,
	PartialEq,
	Eq,
)]
pub struct MetricPoolUpdateOperations {
	pub add: MetricPoolConfigValues,
	pub remove: BoundedVec<MetricPoolConfigName, ConstU32<CONFIG_VALUES_MAX_LENGTH>>,
}

/// A processor's possible stati.
#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	TypeInfo,
	RuntimeDebugNoBound,
	Clone,
	Copy,
	PartialEq,
	Eq,
)]
pub enum ProcessorStatus<BlockNumber: Debug> {
	/// The benchmarked metric was committed but is in warmup and becomes active at the given block number.
	///
	/// We store the block when active instead of when first metric was seen to save on a looking up `warm_up` each time we are checking if active.
	WarmupUntil(BlockNumber),
	/// The benchmarked metric was committed and is active (warmup passed).
	Active,
}

#[derive(
	RuntimeDebugNoBound,
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	TypeInfo,
	Clone,
	PartialEq,
	Eq,
)]
pub struct MetricPool<
	Epoch: Copy + Ord + One + Add<Output = Epoch> + Debug,
	Value: Copy + Default + Debug,
> {
	/// The generic config values for this pool used by benchmarks on processor.
	///
	/// This should be first field to simplify parsing on processor.
	pub config: MetricPoolConfigValues,

	pub name: MetricPoolName,
	pub reward: ProvisionalBuffer<Epoch, Value>,
	pub total: SlidingBuffer<Epoch, FixedU128>,
}

impl<
		Epoch: Copy + Ord + One + Add<Output = Epoch> + Debug,
		Value: Saturating + Copy + Default + Debug,
	> MetricPool<Epoch, Value>
{
	pub fn add(&mut self, epoch: Epoch, summand: FixedU128) {
		self.total.mutate(epoch, |v| {
			*v = v.saturating_add(summand);
		});
	}
}

/// Stores a processor's metric commitment.
#[derive(RuntimeDebugNoBound, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq)]
pub struct MetricCommit<Epoch: Debug> {
	/// The processor epoch number the metric got committed for.
	pub epoch: Epoch,
	/// The metric result.
	pub metric: Metric,
}

#[derive(RuntimeDebugNoBound, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq)]
pub struct ProcessorState<BlockNumber: Debug, Epoch: Debug, Balance: Debug> {
	/// The offset in blocks this processor's epoch has from current global epoch.
	///
	/// **Currently unused**:
	/// It is **not** aligned with global epochs and could be used in the future to ensure commit-claim operations don't overload the chain on same block range of `[global_epoch_start, global_epoch_start + heartbeat_interval]`.
	pub epoch_offset: BlockNumber,
	/// The latest epoch in which a processor committed.
	pub committed: Epoch,
	/// The latest epoch for which a processor claimed.
	pub claimed: Epoch,
	pub status: ProcessorStatus<BlockNumber>,
	/// The amount accrued but not yet paid out.
	///
	/// This is helpful in case the reward transfer fails, we still keep the open amount in accrued.
	///
	/// Also see [`Self.paid`]:
	pub accrued: Balance,
	/// The total amount paid out. There can be additional amounts waiting in [`Self.accrued`] to be paid out.
	pub paid: Balance,
}

/// A manager's commitment of compute and stake to a specific pool.
///
/// The maximum commitment possible to state is `min(commitment, 0.8 * latest-completed-era-average)`.
#[derive(
	RuntimeDebugNoBound,
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	TypeInfo,
	Clone,
	PartialEq,
	Eq,
)]
pub struct ComputeCommitment {
	/// Identifies pool this commitment was made for.
	pub pool_id: PoolId,
	/// Total metric a manager commits to over all his processors to the specific pool.
	pub metric: Metric,
}

/// The state for any staker, both compute provider and delegator.
#[derive(RuntimeDebugNoBound, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq)]
pub struct Stake<Balance: Debug, BlockNumber: Debug> {
	/// The amount delegated.
	pub amount: Balance,
	/// The amount accrued but not yet paid out or compound to amount.
	///
	/// This is helpful in case the reward transfer fails, we still keep the open amount in accrued.
	pub accrued: Balance,
	/// Cooldown period; how long a delegator commits his delegated stake after the block of cooldown initiation.
	///
	/// Cooldown has to be multiple of era length, but is stored in blocks to ensure era length could be adapted.
	/// The cooldown is only started at next era after cooldown initiation.
	///
	/// For delegators' [`StakeState`], the cooldown is always shorter then the cooldown of the delegatee, the compute provider (and staker).
	pub cooldown_period: BlockNumber,
	/// If in cooldown, when the cooldown was initiated.
	pub cooldown_started: Option<BlockNumber>,
}

#[derive(
	RuntimeDebugNoBound, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Default,
)]
pub struct DelegateeTotal<Balance: Debug + Default> {
	pub amount: Balance,
	pub weight: Balance,
	pub count: u8,
}

#[derive(
	RuntimeDebugNoBound, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Default,
)]
pub struct ManagerPreferences {
	#[codec(compact)]
	pub commission: Perbill,
}

#[derive(Clone, PartialEq, Eq)]
pub enum LockReason<ManagerId> {
	Staking,
	Delegation(ManagerId),
}
