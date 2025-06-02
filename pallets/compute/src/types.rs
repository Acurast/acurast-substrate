use core::ops::Add;

use frame_support::pallet_prelude::*;
use sp_runtime::{
	traits::{Debug, One, Saturating},
	FixedU128, Perquintill,
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
	/// The lastest epoch in which a processor committed.
	pub committed: Epoch,
	/// The lastest epoch for which a processor claimed.
	pub claimed: Epoch,
	pub status: ProcessorStatus<BlockNumber>,
	/// The amount accrued but not paid out.
	///
	/// This is helpful in case the reward transfer fails, we still keep the open amount in accrued.
	///
	/// Also see [`Self.paid`]:
	pub accrued: Balance,
	/// The total amount paid out. There can be additional amounts waiting in [`Self.accrued`] to be paid out.
	pub paid: Balance,
}
