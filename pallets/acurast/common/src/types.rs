#[cfg(feature = "attestation")]
mod bounded_attestation;

#[cfg(feature = "attestation")]
pub use bounded_attestation::*;

use frame_support::{pallet_prelude::*, storage::bounded_vec::BoundedVec};
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
    Ethereum(EthereumAddressBytes),
    AlephZero(AcurastAccountId),
}

pub type TezosAddressBytes = BoundedVec<u8, CU32<36>>;
pub type EthereumAddressBytes = BoundedVec<u8, CU32<20>>;

/// The type of a job identifier sequence.
pub type JobIdSequence = u128;

/// A Job ID consists of a [MultiOrigin] and a job identifier respective to the source chain.
pub type JobId<AcurastAccountId> = (MultiOrigin<AcurastAccountId>, JobIdSequence);

/// The allowed sources update operation.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Copy)]
pub enum ListUpdateOperation {
    Add,
    Remove,
}

#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
pub struct ListUpdate<T>
where
    T: Encode + Decode + TypeInfo + MaxEncodedLen + Clone + PartialEq,
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
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
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

pub const PUB_KEYS_MAX_LENGTH: u32 = 33;
pub type PubKeyBytes = BoundedVec<u8, ConstU32<PUB_KEYS_MAX_LENGTH>>;

/// Structure representing execution environment variables encrypted for a specific processor.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
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
    pub variables:
        BoundedVec<(BoundedVec<u8, KeyMaxSize>, BoundedVec<u8, ValueMaxSize>), MaxEnvVars>,
}

pub const MAX_JOB_MODULES: u32 = 1;

#[derive(
    RuntimeDebug,
    Encode,
    Decode,
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

    /// Range of a schedule from first execution's start to end of last execution, respecting `start_delay`.
    ///
    /// Example:
    /// ___□□■■_□□■■_□□■■__.range(2) -> (3, 17)
    pub fn range(&self, start_delay: u64) -> Option<(u64, u64)> {
        let actual_start = self.start_time.checked_add(start_delay)?;
        let count = self.execution_count();
        let actual_end = if count > 0 {
            actual_start
                .checked_add((count - 1).checked_mul(self.interval)?)?
                .checked_add(self.duration)?
        } else {
            actual_start
        };
        Some((actual_start, actual_end))
    }

    pub fn overlaps(&self, start_delay: u64, a: u64, b: u64) -> Option<bool> {
        let (start, end) = self.range(start_delay)?;
        if b <= a || start == end || b <= start || end <= a {
            return Some(false);
        }

        // if query interval `[a, b]` starts before, we can pretend it only starts at `start`
        let relative_a = a.checked_sub(start).unwrap_or(start);

        if let Some(relative_b) = b.checked_sub(start) {
            let a = relative_a % self.interval;
            let _b = relative_b % self.interval;
            let b = if _b == 0 { self.interval } else { _b };

            let l = b.checked_sub(a).unwrap_or(0);
            //   ╭a    ╭b
            // ■■■■______■■■■______
            // OR
            //   ╭b  ╭a    ╭b'
            // ■■■■______■■■■______
            Some(b < a || a < self.duration || l >= self.interval)
        } else {
            Some(false)
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

impl<'a> Iterator for ScheduleIter {
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
            }
            Some(curr) => {
                let next = curr.checked_add(self.interval)?;
                if next < self.delayed_end_time {
                    Some(next)
                } else {
                    None
                }
            }
        };
        self.current
    }
}

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
