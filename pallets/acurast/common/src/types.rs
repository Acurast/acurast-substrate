mod account20;

#[cfg(feature = "attestation")]
mod bounded_attestation;

#[cfg(feature = "attestation")]
pub use bounded_attestation::*;

pub use account20::*;

use frame_support::{pallet_prelude::*, sp_runtime::FixedU128, storage::bounded_vec::BoundedVec};
use sp_core::crypto::AccountId32;
use sp_std::prelude::*;

use crate::ParameterBound;
use serde::{Deserialize, Serialize};

pub(crate) const SCRIPT_PREFIX: &[u8] = b"ipfs://";
pub(crate) const SCRIPT_LENGTH: u32 = 53;

/// Type representing the utf8 bytes of a string containing the value of an ipfs url.
/// The ipfs url is expected to point to a script.
pub type Script = BoundedVec<u8, ConstU32<SCRIPT_LENGTH>>;
pub type AllowedSources<AccountId, MaxAllowedSources> = BoundedVec<AccountId, MaxAllowedSources>;

pub fn is_valid_script(script: &Script) -> bool {
	let script_len: u32 = script.len().try_into().unwrap_or(0);
	script_len == SCRIPT_LENGTH && script.starts_with(SCRIPT_PREFIX)
}

/// https://datatracker.ietf.org/doc/html/rfc5280#section-4.1.2.2
const SERIAL_NUMBER_MAX_LENGTH: u32 = 20;

pub type SerialNumber = BoundedVec<u8, ConstU32<SERIAL_NUMBER_MAX_LENGTH>>;

/// A multi origin identifies a given address from a given origin chain.
#[derive(
	RuntimeDebug,
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	TypeInfo,
	Clone,
	Eq,
	PartialEq,
	Serialize,
	Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum MultiOrigin<AcurastAccountId> {
	Acurast(AcurastAccountId),
	Tezos(TezosAddressBytes),
	// TODO #[deprecated(note = "use `Ethereum20` instead")]
	Ethereum(EthereumAddressBytes),
	AlephZero(AcurastAccountId),
	Vara(AcurastAccountId),
	Ethereum20(AccountId20),
	Solana(AccountId32),
}

/// The proxy describes the chain where there is a counter part to this pallet, processing messages we send and also sending messages back. This mostly will be a custom _Hyperdrive Token_ contract on the proxy chain.
#[derive(
	RuntimeDebug,
	Encode,
	Decode,
	DecodeWithMemTracking,
	TypeInfo,
	Copy,
	Clone,
	Eq,
	PartialEq,
	MaxEncodedLen,
)]
pub enum ProxyChain {
	Acurast,
	Tezos,
	AlephZero,
	Vara,
	Ethereum,
	Solana,
}

impl<AcurastAccountId> From<&MultiOrigin<AcurastAccountId>> for ProxyChain {
	fn from(origin: &MultiOrigin<AcurastAccountId>) -> Self {
		match origin {
			MultiOrigin::Acurast(_) => ProxyChain::Acurast,
			MultiOrigin::Tezos(_) => ProxyChain::Tezos,
			MultiOrigin::AlephZero(_) => ProxyChain::AlephZero,
			MultiOrigin::Vara(_) => ProxyChain::Vara,
			MultiOrigin::Ethereum(_) | MultiOrigin::Ethereum20(_) => ProxyChain::Ethereum,
			MultiOrigin::Solana(_) => ProxyChain::Solana,
		}
	}
}

pub type TezosAddressBytes = BoundedVec<u8, CU32<36>>;
pub type EthereumAddressBytes = BoundedVec<u8, CU32<20>>;

/// The type of a job identifier sequence.
pub type JobIdSequence = u128;

/// A Job ID consists of a [MultiOrigin] and a job identifier respective to the source chain.
pub type JobId<AcurastAccountId> = (MultiOrigin<AcurastAccountId>, JobIdSequence);

/// The allowed sources update operation.
#[derive(
	RuntimeDebug,
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	TypeInfo,
	Clone,
	PartialEq,
	Copy,
)]
pub enum ListUpdateOperation {
	Add,
	Remove,
}

#[derive(
	RuntimeDebug, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq,
)]
pub struct ListUpdate<T>
where
	T: Encode + Decode + DecodeWithMemTracking + TypeInfo + MaxEncodedLen + Clone + PartialEq,
{
	/// The update operation.
	pub operation: ListUpdateOperation,
	pub item: T,
}

/// Structure used to updated the allowed sources list of a [Registration].
pub type AllowedSourcesUpdate<AccountId> = ListUpdate<AccountId>;

/// Structure used to updated the certificate recovation list.
pub type CertificateRevocationListUpdate = ListUpdate<SerialNumber>;

/// Structure representing a job registration.
#[derive(
	RuntimeDebug, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct JobRegistration<AccountId, MaxAllowedSources: Get<u32>, Extra> {
	/// The script to execute. It is a vector of bytes representing a utf8 string. The string needs to be a ipfs url that points to the script.
	pub script: Script,
	/// An optional array of the [AccountId]s allowed to fulfill the job. If the array is [None], then all sources are allowed.
	pub allowed_sources: Option<AllowedSources<AccountId, MaxAllowedSources>>,
	/// A boolean indicating if only verified sources can fulfill the job. A verified source is one that has provided a valid key attestation.
	pub allow_only_verified_sources: bool,
	/// The schedule describing the desired (multiple) execution(s) of the script.
	pub schedule: Schedule,
	/// Maximum memory bytes used during a single execution of the job.
	pub memory: u32,
	/// Maximum network request used during a single execution of the job.
	pub network_requests: u32,
	/// Maximum storage bytes used during the whole period of the job's executions.
	pub storage: u32,
	/// The modules required for the job.
	pub required_modules: JobModules,
	/// Extra parameters. This type can be configured through [Config::RegistrationExtra].
	pub extra: Extra,
}

/// Types of script mutability to choose from during registration of a job.
#[derive(
	RuntimeDebug,
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	TypeInfo,
	Clone,
	PartialEq,
	Eq,
	Serialize,
	Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum ScriptMutability<AccountId> {
	/// Creates an immutable job whose script cannot be modified — neither by updating the job nor by registering a new one.
	Immutable,
	/// Creates a mutable job given an optional distinct `editor` from `owner`.
	///
	/// If not provided, the `editor` defaults to the job owner or the job keys are transferred from.
	Mutable(Option<AccountId>),
}

pub const PUB_KEYS_MAX_LENGTH: u32 = 33;
pub type PubKeyBytes = BoundedVec<u8, ConstU32<PUB_KEYS_MAX_LENGTH>>;
pub type EnvVarKey<KeyMaxSize> = BoundedVec<u8, KeyMaxSize>;
pub type EnvVarValue<ValueMaxSize> = BoundedVec<u8, ValueMaxSize>;

/// Structure representing execution environment variables encrypted for a specific processor.
#[derive(
	RuntimeDebug, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct Environment<
	MaxEnvVars: ParameterBound,
	KeyMaxSize: ParameterBound,
	ValueMaxSize: ParameterBound,
> {
	/// Public key of key pair specifically created to encrypt environment secrets.
	pub public_key: PubKeyBytes,
	/// Environment variables with cleartext key, encrypted value.
	pub variables: BoundedVec<(EnvVarKey<KeyMaxSize>, EnvVarValue<ValueMaxSize>), MaxEnvVars>,
}

pub const MAX_JOB_MODULES: u32 = 2;

#[derive(
	RuntimeDebug,
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	TypeInfo,
	Clone,
	Copy,
	PartialEq,
	Eq,
	Serialize,
	Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum JobModule {
	DataEncryption,
	LLM,
}

impl TryFrom<u32> for JobModule {
	type Error = ();

	fn try_from(value: u32) -> Result<Self, Self::Error> {
		match value {
			0 => Ok(JobModule::DataEncryption),
			_ => Err(()),
		}
	}
}

pub type JobModules = BoundedVec<JobModule, ConstU32<MAX_JOB_MODULES>>;

/// The desired schedule with some planning flexibility offered through `max_start_delay`.
///
/// ## Which planned schedules are valid?
///
/// Given `max_start_delay = 8`, `duration = 3`, `interval = 20`:
///
/// * planned delay is constant within the executions *of one slot*
///   ```ignore
///   SLOT 1: □□□□□□■■■□__________□□□□□□■■■□__________□□□□□□■■■□
///   SLOT 2: ■■■□□□□□□□__________■■■□□□□□□□__________■■■□□□□□□□
///   SLOT 3: □□■■■□□□□□__________□□■■■□□□□□__________□□■■■□□□□□
///   ```
#[derive(
	RuntimeDebug,
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	TypeInfo,
	Clone,
	Eq,
	PartialEq,
	Serialize,
	Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct Schedule {
	/// An upperbound for the duration of one execution of the script in milliseconds.
	pub duration: u64,
	/// Start time in milliseconds since Unix Epoch.
	pub start_time: u64,
	/// End time in milliseconds since Unix Epoch.
	///
	/// Represents the end time (exclusive) in milliseconds since Unix Epoch
	/// of the period in which a job execution can start, relative to `start_delay == 0`, independent of `duration`.
	///
	/// Hence the latest possible start time is `end_time + start_delay - 1`.
	/// and all executions fit into `[start_time + start_delay, end_time + duration + start_delay]`.
	///
	/// (start_delay is the actual start delay chosen within `[0, max_start_delay]` during assigning the job to an available processor)
	pub end_time: u64,
	/// Interval at which to repeat execution in milliseconds.
	pub interval: u64,
	/// Maximum delay before each execution in milliseconds.
	pub max_start_delay: u64,
}

impl Schedule {
	/// The number of executions in the [`Schedule`] which corresponds to the length of [`Schedule::iter()`].
	pub fn execution_count(&self) -> u64 {
		(|| -> Option<u64> {
			self.end_time
				.checked_sub(self.start_time)?
				.checked_sub(1u64)?
				.checked_div(self.interval)?
				.checked_add(1u64)
		})()
		.unwrap_or(0u64)
	}

	/// Iterates over the start times of all the [`Schedule`]'s executions.
	///
	/// All executions fit into `[start_time, end_time + duration + start_delay]`.
	/// Note that the last execution starts before `end_time` but may reach over it.
	/// This is so that *the number of executions does not depend on `start_delay`*.
	pub fn iter(&self, start_delay: u64) -> Option<ScheduleIter> {
		Some(ScheduleIter {
			delayed_start_time: self.start_time.checked_add(start_delay)?,
			delayed_end_time: self.end_time.checked_add(start_delay)?,
			interval: self.interval,
			current: None,
		})
	}

	pub fn nth_start_time(&self, start_delay: u64, execution_index: u64) -> Option<u64> {
		if execution_index >= self.execution_count() {
			return None;
		}
		self.start_time
			.checked_add(start_delay)?
			.checked_add(self.interval.checked_mul(execution_index)?)
	}

	pub fn next_execution_index(&self, start_delay: u64, now: u64) -> u64 {
		self.current_execution_index(start_delay, now)
			.map(|value| value + 1)
			.unwrap_or(0)
	}

	pub fn current_execution_index(&self, start_delay: u64, now: u64) -> Option<u64> {
		let actual_start = self.start_time.saturating_add(start_delay);
		if now < actual_start {
			return None;
		}
		let max_index = self.execution_count() - 1;
		Some(((now - actual_start) / self.interval).min(max_index))
	}

	pub fn actual_start(&self, start_delay: u64) -> u64 {
		self.start_time.saturating_add(start_delay)
	}

	pub fn actual_end(&self, actual_start: u64) -> u64 {
		let count = self.execution_count();
		if count > 0 {
			actual_start
				.saturating_add((count - 1).saturating_mul(self.interval))
				.saturating_add(self.duration)
		} else {
			actual_start
		}
	}

	/// Range of a schedule from first execution's start to end of last execution, respecting `start_delay`.
	///
	/// Example:
	/// ___□□■■_□□■■_□□■■__.range(2) -> (3, 17)
	pub fn range(&self, start_delay: u64) -> (u64, u64) {
		let actual_start = self.actual_start(start_delay);
		let actual_end = self.actual_end(actual_start);
		(actual_start, actual_end)
	}

	pub fn overlaps(&self, start_delay: u64, bounds: (u64, u64)) -> bool {
		let (a, b) = bounds;
		let (start, end) = self.range(start_delay);
		if b <= a || start == end || b <= start || end <= a {
			return false;
		}

		// if query interval `[a, b]` starts before, we can pretend it only starts at `start`
		let relative_a = a.checked_sub(start).unwrap_or(start);

		if let Some(relative_b) = b.checked_sub(start) {
			let a = relative_a % self.interval;
			let _b = relative_b % self.interval;
			let b = if _b == 0 { self.interval } else { _b };

			let l = b.saturating_sub(a);
			//   ╭a    ╭b
			// ■■■■______■■■■______
			// OR
			//   ╭b  ╭a    ╭b'
			// ■■■■______■■■■______
			b < a || a < self.duration || l >= self.interval
		} else {
			false
		}
	}
}

/// Implements the [Iterator] trait so that scheduled jobs in a [Schedule] can be iterated.
pub struct ScheduleIter {
	delayed_start_time: u64,
	delayed_end_time: u64,
	interval: u64,
	current: Option<u64>,
}

impl Iterator for ScheduleIter {
	type Item = u64;

	// Here, we define the sequence using `.current` and `.next`.
	// The return type is `Option<T>`:
	//     * When the `Iterator` is finished, `None` is returned.
	//     * Otherwise, the next value is wrapped in `Some` and returned.
	// We use Self::Item in the return type, so we can change
	// the type without having to update the function signatures.
	fn next(&mut self) -> Option<Self::Item> {
		self.current = match self.current {
			None => {
				if self.delayed_start_time < self.delayed_end_time {
					Some(self.delayed_start_time)
				} else {
					None
				}
			},
			Some(curr) => {
				let next = curr.checked_add(self.interval)?;
				if next < self.delayed_end_time {
					Some(next)
				} else {
					None
				}
			},
		};
		self.current
	}
}

#[derive(
	RuntimeDebug,
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	TypeInfo,
	Clone,
	Copy,
	PartialEq,
	Eq,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct Version {
	/// Number representing the device's platform:
	/// 0: Android
	pub platform: u32,
	pub build_number: u32,
}

impl PartialOrd for Version {
	fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
		match self.platform.partial_cmp(&other.platform) {
			Some(core::cmp::Ordering::Equal) => {},
			_ => return None,
		}
		self.build_number.partial_cmp(&other.build_number)
	}
}

/// Type used for unique identifier of each pool.
pub type PoolId = u8;

/// A metric specified as `(pool_id, numerator, denominator)`.
///
/// A list of metrics are committed after deriving them from performed benchmarks on processor.
///
/// The metric is transformed into a [`FixedU128`] defined by `numerator / denominator`.
pub type MetricInput = (PoolId, u128, u128);

pub const METRICS_MAX_LENGTH: u32 = 20;

/// A list of benchmarked values of a processor for a (sub)set of known metrics.
///
/// Specified as `(pool_name, numerator, denominator)`.
pub type Metrics = BoundedVec<MetricInput, ConstU32<METRICS_MAX_LENGTH>>;

#[derive(
	RuntimeDebug,
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	TypeInfo,
	Clone,
	Copy,
	PartialEq,
	Eq,
)]
pub struct MinMetric {
	pub pool_id: PoolId,
	pub value: FixedU128,
}

impl From<MetricInput> for MinMetric {
	fn from(value: MetricInput) -> Self {
		let (pool_id, numerator, denominator) = value;
		let metric = FixedU128::from_rational(
			numerator,
			if denominator.is_zero() { One::one() } else { denominator },
		);
		Self { pool_id, value: metric }
	}
}

pub type MinMetrics = BoundedVec<MinMetric, ConstU32<METRICS_MAX_LENGTH>>;

#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, Eq, PartialEq)]
pub struct CU32<const T: u32>;
impl<const T: u32> Get<u32> for CU32<T> {
	fn get() -> u32 {
		T
	}
}

impl<const T: u32> Get<Option<u32>> for CU32<T> {
	fn get() -> Option<u32> {
		Some(T)
	}
}

impl<const T: u32> TypedGet for CU32<T> {
	type Type = u32;
	fn get() -> u32 {
		T
	}
}

#[cfg(feature = "std")]
impl<const T: u32> Serialize for CU32<T> {
	fn serialize<D>(&self, serializer: D) -> Result<D::Ok, D::Error>
	where
		D: serde::Serializer,
	{
		serializer.serialize_u32(<Self as TypedGet>::get())
	}
}

#[cfg(feature = "std")]
impl<'de, const T: u32> Deserialize<'de> for CU32<T> {
	fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		Ok(CU32::<T>)
	}
}
