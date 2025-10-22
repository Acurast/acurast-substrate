use core::ops::Add;

use acurast_common::PoolId;
use frame_support::{pallet_prelude::*, traits::Currency};
use frame_system::pallet_prelude::BlockNumberFor;
use sp_core::U256;
use sp_runtime::{
	traits::{Debug, One, Saturating, Zero},
	FixedU128, Perbill, Perquintill,
};

use crate::{
	datastructures::{ProvisionalBuffer, SlidingBuffer},
	Config, MemoryBuffer,
};

pub type EpochOf<T> = BlockNumberFor<T>;
pub type EraOf<T> = BlockNumberFor<T>;
pub type CycleFor<T> = Cycle<EpochOf<T>, BlockNumberFor<T>>;
pub type MetricPoolFor<T> = MetricPool<EpochOf<T>, Perquintill>;
pub type ProcessorStateFor<T, I> =
	ProcessorState<BlockNumberFor<T>, BlockNumberFor<T>, BalanceFor<T, I>>;
pub type ProcessorStatusFor<T> = ProcessorStatus<BlockNumberFor<T>>;
pub type MetricCommitFor<T> = MetricCommit<BlockNumberFor<T>>;

pub const CONFIG_VALUES_MAX_LENGTH: u32 = 20;
/// Precision constant for U256 calculations (10^30)
pub const PER_TOKEN_DECIMALS: u128 = 1_000_000_000_000_000_000_000_000_000_000;
pub const FIXEDU128_DECIMALS: u128 = 1_000_000_000_000_000_000;
pub type MetricPoolConfigValues =
	BoundedVec<MetricPoolConfigValue, ConstU32<CONFIG_VALUES_MAX_LENGTH>>;

pub type MetricPoolConfigValue = (MetricPoolConfigName, u128, u128);

/// The type of a metric.
pub type Metric = FixedU128;

/// The type of a metric pool's name.
pub type MetricPoolName = [u8; 24];

pub type MetricPoolConfigName = [u8; 24];

pub type StakeFor<T, I> = Stake<BalanceFor<T, I>, BlockNumberFor<T>>;
pub type CommitmentFor<T, I> = Commitment<BalanceFor<T, I>, BlockNumberFor<T>, EpochOf<T>>;
pub type DelegationFor<T, I> = Delegation<BalanceFor<T, I>, BlockNumberFor<T>>;
pub type RewardBudgetFor<T, I> = RewardBudget<BalanceFor<T, I>>;

#[derive(
	RuntimeDebug,
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	TypeInfo,
	Copy,
	Clone,
	PartialEq,
	Eq,
	Default,
)]
pub struct RewardBudget<Balance> {
	/// The total reward budget.
	pub total: Balance,
	/// The amount that got distributed, not necessarily claimed but claimable from now onwards.
	pub distributed: Balance,
	pub target_weight_per_compute: U256,

	/// The total score (adjusted_weight * reported_metric) as a running sum over all committers to a pool.
	pub total_score: U256,
}

impl<Balance: Debug + Zero + Copy> RewardBudget<Balance> {
	pub fn new(total: Balance, target_weight_per_compute: U256) -> Self {
		Self {
			total,
			distributed: Zero::zero(),
			target_weight_per_compute,
			total_score: Zero::zero(),
		}
	}
}

#[derive(
	RuntimeDebug,
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	TypeInfo,
	Copy,
	Clone,
	PartialEq,
	Eq,
	Default,
)]
pub struct Cycle<Epoch, BlockNumber> {
	pub epoch: Epoch,
	pub epoch_start: BlockNumber,
}

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
	RuntimeDebug,
	Clone,
	Copy,
	PartialEq,
	Eq,
)]
pub enum ProcessorStatus<BlockNumber> {
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
		self.total.mutate(
			epoch,
			|v| {
				*v = v.saturating_add(summand);
			},
			false,
		);
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

impl<BlockNumber: Debug, Epoch: Debug, Balance: Debug> ProcessorState<BlockNumber, Epoch, Balance>
where
	BlockNumber: Zero,
	Balance: Zero,
	Epoch: Zero,
{
	pub fn initial(epoch_offset: BlockNumber, warmup_end: BlockNumber) -> Self {
		Self {
			// currently unused, see comment why we initialize this anyways
			epoch_offset,
			committed: Zero::zero(),
			claimed: Zero::zero(),
			status: ProcessorStatus::WarmupUntil(warmup_end),
			accrued: Zero::zero(),
			paid: Zero::zero(),
		}
	}
}

#[derive(
	Debug, Clone, PartialEq, Eq, Encode, Decode, MaxEncodedLen, TypeInfo, DecodeWithMemTracking,
)]
pub struct RewardSettings<Balance, AccountId> {
	/// The fixed reward distributed per epoch.
	///
	/// This amount is not minted but just taken from [`Self::distribution_account`] every epoch.
	///
	/// This is intended to be eventually zeroed in favor of purely inflation based rewards.
	pub total_reward_per_distribution: Balance,
	/// The inflation-based reward distributed per epoch as a ratio of total supply.
	///
	/// This amount is minted into  [`Self::distribution_account`].
	/// A part might be actively distributed immediately after minting while another part resides on the distribution account ready for withdraw operations of staking system.
	pub total_inflation_per_distribution: Perquintill,
	/// The ratio of inflation-created reward that is distributed to stake-backed compute providers using commitment and delegation pools.
	pub stake_backed_ratio: Perquintill,
	/// The account to distribute from.
	pub distribution_account: AccountId,
}

pub type BalanceFor<T, I> =
	<<T as Config<I>>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

pub type RewardDistributionSettingsFor<T, I> =
	RewardSettings<BalanceFor<T, I>, <T as frame_system::Config>::AccountId>;

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

/// The stake details, both for compute provider staking and any account delegating.
#[derive(RuntimeDebugNoBound, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq)]
pub struct Stake<Balance: Debug, BlockNumber: Debug> {
	/// The amount.
	pub amount: Balance,
	/// The rewardable_amount amount, used to separate the reduced amount in cooldown from original amount. It always holds [`Self::rewardable_amount`] <= [`Self::amount`].
	pub rewardable_amount: Balance,
	/// The block number when the stake was created.
	pub created: BlockNumber,
	/// Cooldown period; how long a delegator commits his delegated stake after the block of cooldown initiation.
	///
	/// Cooldown has to be multiple of era length, but is stored in blocks to ensure era length could be adapted.
	/// The cooldown is only started at next era after cooldown initiation.
	///
	/// For delegators' [`StakeState`], the cooldown is always shorter then the cooldown of the delegatee, the compute provider (and staker).
	pub cooldown_period: BlockNumber,
	/// If in cooldown, when the cooldown was initiated.
	pub cooldown_started: Option<BlockNumber>,
	/// The amount accrued but not yet paid out or compounded to amount.
	///
	/// This is also helpful in case the reward transfer fails, we still keep the open amount in accrued.
	pub accrued_reward: Balance,
	/// The slash accrued but not yet paid out or compounded to amount.
	///
	/// Any further compound or payout operations must be preceeded by accruing potentially outstanding slash debt.
	pub accrued_slash: Balance,
	/// If any account (such as a courtesy service by Acurast) is allowed to compound for this stake.
	pub allow_auto_compound: bool,
	/// The total amount paid out or compounded. There can be additional amounts waiting in [`Self.accrued_reward`] to be paid out.
	pub paid: Balance,
	/// The total slash ever applied to stake or to compounded stake. There can be additional amounts waiting in [`Self.accrued_slash`] to be paid out.
	pub applied_slash: Balance,
}

impl<Balance: Debug + Zero + Copy, BlockNumber: Debug> Stake<Balance, BlockNumber> {
	pub fn new(
		amount: Balance,
		created: BlockNumber,
		cooldown_period: BlockNumber,
		allow_auto_compound: bool,
	) -> Self {
		Self {
			amount,
			rewardable_amount: amount,
			created,
			cooldown_period,
			cooldown_started: None,
			accrued_reward: Zero::zero(),
			accrued_slash: Zero::zero(),
			paid: Zero::zero(),
			applied_slash: Zero::zero(),
			allow_auto_compound,
		}
	}
}

/// The commitment state of a committer including his stake details.
#[derive(RuntimeDebugNoBound, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq)]
pub struct Commitment<
	Balance: Debug,
	BlockNumber: Debug + Ord + Copy,
	Epoch: Debug + Ord + Copy + One + Add<Output = Epoch>,
> {
	pub stake: Option<Stake<Balance, BlockNumber>>,

	/// The relative commission taken by committers from delegator's reward as [`Perbill`].
	pub commission: Perbill,

	pub delegations_total_amount: Balance,
	pub delegations_total_rewardable_amount: Balance,

	/// The commitments weights as a MemoryBuffer `[epoch -> weights]`.
	///
	///     e1        e2       e3          e4
	/// |--------|----------|--D-----|
	///                        |
	///                        D first write rotates, but keeps old value as base
	///                        clear out before e2
	///                        #
	///                 |
	///          
	///           +$ +$ # $ (problematic in e2)   
	///                 |
	///                 first heartbeat scores for score_committer_c out from delegations_reward_weight from e1
	pub weights: MemoryBuffer<Epoch, CommitmentWeights>,

	/// The pool rewards as a MemoryBuffer to remember it for maximum one past commitment.
	///
	/// The time unit is the created timestamp of the commitment, `stake.created`.
	/// This allows to distinguish for a delegator operation if the value in `past` of MemoryBuffer was
	/// for the commitment a delegator choose or if the delegator's commitment was already "phased out",
	/// which is the case if his `created` timestamp is even before that of `past`.
	pub pool_rewards: MemoryBuffer<BlockNumber, PoolReward>,

	/// The epoch number in which any processor belonging to this commitment did calculate scores.
	pub last_scoring_epoch: Epoch,
}

#[derive(
	RuntimeDebugNoBound,
	Encode,
	Decode,
	MaxEncodedLen,
	TypeInfo,
	Clone,
	Copy,
	PartialEq,
	Eq,
	Default,
)]
pub struct CommitmentWeights {
	pub self_reward_weight: U256,
	pub self_slash_weight: U256,

	pub delegations_reward_weight: U256,
	pub delegations_slash_weight: U256,
}

#[derive(
	RuntimeDebugNoBound,
	Encode,
	Decode,
	MaxEncodedLen,
	TypeInfo,
	Clone,
	Copy,
	PartialEq,
	Eq,
	Default,
)]
pub struct PoolReward {
	pub reward_per_weight: U256,
	pub slash_per_weight: U256,
}

impl CommitmentWeights {
	pub fn total_reward_weight(&self) -> U256 {
		self.self_reward_weight.saturating_add(self.delegations_reward_weight)
	}

	pub fn total_slash_weight(&self) -> U256 {
		self.self_slash_weight.saturating_add(self.delegations_slash_weight)
	}
}

/// The state of a delegator including his stake details.
#[derive(RuntimeDebugNoBound, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq)]
pub struct Delegation<Balance: Debug, BlockNumber: Debug> {
	pub stake: Stake<Balance, BlockNumber>,

	/// The weight for rewardability, i.e. balance weighted by one or several factors.
	pub reward_weight: U256,
	/// The weight for slashability, i.e. balance weighted by one or several factors.
	pub slash_weight: U256,
	/// The weighted reward debt before this staker joined a pool.
	pub reward_debt: Balance,
	/// The weighted slash debt before this staker joined a pool.
	pub slash_debt: Balance,
}

#[derive(Clone, PartialEq, Eq)]
pub enum LockReason<ManagerId> {
	Staking,
	Delegation(ManagerId),
}
