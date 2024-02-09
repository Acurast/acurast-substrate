use core::marker::PhantomData;

use codec::{Decode, Encode};
#[cfg(feature = "std")]
use derive_more::Error as DError;
use derive_more::{Display, From};
use once_cell::race::OnceBox;
use scale_info::TypeInfo;
use sp_core::bounded::BoundedVec;
use sp_core::{RuntimeDebug, H256};
use sp_runtime::traits::Hash;
use sp_std::prelude::*;
use sp_std::str::FromStr;
use sp_std::vec;
use tezos_core::types::encoded::Address as TezosAddress;
use tezos_core::Error as TezosCoreError;
use tezos_michelson::micheline::primitive_application::PrimitiveApplication;
use tezos_michelson::michelson::data::{
    self, try_bytes, try_int, try_nat, try_string, Bytes, Data, Elt, Int, Nat, Pair, Sequence,
};
use tezos_michelson::michelson::types::{
    address, bool as bool_type, bytes, map, nat, option, pair, set, string,
};
use tezos_michelson::Error as TezosMichelineError;
use tezos_michelson::{
    micheline::{primitive_application, Micheline},
    michelson::ComparableTypePrimitive,
};

use pallet_acurast::{
    AllowedSources, Environment, EnvironmentFor, JobIdSequence, JobModule, JobRegistration,
    MultiOrigin, ParameterBound, Schedule, CU32,
};
use pallet_acurast_marketplace::{
    JobRequirements, PlannedExecution, PlannedExecutions, RegistrationExtra,
};

use crate::types::{
    derive_proof, MessageParser, RawAction, StateKey, StateOwner, StateProof, StateValue,
};
use crate::{traits, CurrentTargetChainOwner, MessageIdentifier, ParsedAction};

pub struct TezosParser<T, I, ParsableAccountId>(PhantomData<(T, I, ParsableAccountId)>);

impl<T, I: 'static, ParsableAccountId> MessageParser<T> for TezosParser<T, I, ParsableAccountId>
where
    T: crate::pallet::Config<I>,
    T::RegistrationExtra: From<RegistrationExtra<T::Balance, T::AccountId, T::MaxSlots>>,
    ParsableAccountId: TryFrom<Vec<u8>> + Into<T::AccountId>,
{
    type Error = TezosValidationError;

    /// Parses an encoded key from Tezos representing a message identifier.
    fn parse_key(encoded: &[u8]) -> Result<MessageIdentifier, Self::Error> {
        let schema = primitive_application(ComparableTypePrimitive::Nat).into();
        let micheline: Micheline = Micheline::unpack(encoded, Some(&schema))
            .map_err(|e| TezosValidationError::TezosMicheline(e))?;

        let value: Nat = micheline.try_into()?;
        value
            .to_integer()
            .map_err(|_| TezosValidationError::ParsingFailure)
    }

    fn parse_value(encoded: &[u8]) -> Result<ParsedAction<T>, TezosValidationError> {
        let (action, origin, payload) = parse_message(encoded)?;

        Ok(match action {
            RawAction::RegisterJob => {
                let payload: Vec<u8> = (&payload).into();
                let (job_id_sequence, registration) = parse_job_registration_payload::<
                    T::Balance,
                    ParsableAccountId,
                    T::AccountId,
                    T::MaxAllowedSources,
                    T::MaxSlots,
                    T::RegistrationExtra,
                >(payload.as_slice())?;

                ParsedAction::RegisterJob(
                    (
                        MultiOrigin::Tezos(bounded_address(&origin)?),
                        job_id_sequence,
                    ),
                    registration,
                )
            }
            RawAction::DeregisterJob => {
                let payload: Vec<u8> = (&payload).into();
                let job_id_sequence = parse_deregister_job_payload(payload.as_slice())?;

                ParsedAction::DeregisterJob((
                    MultiOrigin::Tezos(bounded_address(&origin)?),
                    job_id_sequence,
                ))
            }
            RawAction::FinalizeJob => {
                let payload: Vec<u8> = (&payload).into();
                let job_ids = parse_finalize_job_payload(payload.as_slice())?;

                let address = bounded_address(&origin)?;
                ParsedAction::FinalizeJob(
                    job_ids
                        .into_iter()
                        .map(|job_id_seq| (MultiOrigin::Tezos(address.clone()), job_id_seq))
                        .collect(),
                )
            }
            RawAction::SetJobEnvironment => {
                let payload: Vec<u8> = (&payload).into();
                let (job_id_sequence, set_job_environment) =
                    parse_set_job_environment_payload::<T, ParsableAccountId>(payload.as_slice())?;

                let job_id = (
                    MultiOrigin::Tezos(bounded_address(&origin)?),
                    job_id_sequence,
                );

                ParsedAction::SetJobEnvironment(job_id, set_job_environment)
            }
            RawAction::Noop => ParsedAction::Noop,
        })
    }
}

#[cfg_attr(rustfmt, rustfmt::skip)]
fn message_schema() -> &'static Micheline {
    static MESSAGE_SCHEMA: OnceBox<Micheline> = OnceBox::new();
    MESSAGE_SCHEMA.get_or_init(|| {
        let schema: Micheline = pair(vec![
            // ACTION NAME
            string(),
            // TEZOS ORIGIN
            address(),
            // ACTION PAYLOAD
            bytes(),
        ]);
        Box::new(schema)
    })
}

/// Parses an encoded message from Tezos representing an action into a tuple `(ACTION, ORIGIN, PAYLOAD)`.
///
/// # Example
/// A message to register a job could look like:
///
fn parse_message(encoded: &[u8]) -> Result<(RawAction, TezosAddress, Bytes), TezosValidationError> {
    let unpacked: Micheline = Micheline::unpack(encoded, Some(message_schema()))
        .map_err(|e| TezosValidationError::TezosMicheline(e))?;

    let p: PrimitiveApplication = unpacked.try_into()?;
    let pair: Pair = p.try_into()?;

    let values = pair.flatten().values;
    if values.len() != 3 {
        Err(TezosValidationError::InvalidMessage)?;
    }
    let mut iter = values.into_iter();

    let action = {
        let action: data::String = iter
            .next()
            .ok_or(TezosValidationError::MissingField(FieldError::ACTION))?
            .try_into()?;
        RawAction::from_str(action.to_str()).map_err(|_| TezosValidationError::InvalidAction)?
    };
    let origin: TezosAddress = try_address(
        iter.next()
            .ok_or(TezosValidationError::MissingField(FieldError::ORIGIN))?,
    )?;
    let body: Bytes = try_bytes(
        iter.next()
            .ok_or(TezosValidationError::MissingField(FieldError::PAYLOAD))?,
    )?;

    Ok((action, origin, body))
}

/// The structure of a [`RawAction::RegisterJob`] action before flattening:
///
/// ```txt
/// sp.TRecord(
///     allowOnlyVerifiedSources=sp.TBool,
///     allowedSources=sp.TOption(sp.TSet(sp.TString)),
///     extra=sp.TRecord(
///         requirements=sp.TRecord(
///             instantMatch=sp.TOption(
///                 sp.TSet(
///                     sp.TRecord(
///                         source=sp.TString,
///                         startDelay=sp.TNat,
///                     )
///                 )
///             ),
///             minReputation=sp.TOption(sp.TNat),
///             reward=sp.TNat,
///             slots=sp.TNat,
///         ).right_comb(),
///     ).right_comb(),
///     jobId=sp.TNat,
///     memory=sp.TNat,
///     networkRequests=sp.TNat,
///     requiredModules = sp.TSet(sp.TNat),
///     schedule=sp.TRecord(
///         duration=sp.TNat,
///         endTime=sp.TNat,
///         interval=sp.TNat,
///         maxStartDelay=sp.TNat,
///         startTime=sp.TNat,
///     ).right_comb(),
///     script=sp.TBytes,
///     storage=sp.TNat,
/// ).right_comb()
/// ```
#[cfg_attr(rustfmt, rustfmt::skip)]
fn registration_payload_schema() -> &'static Micheline {
    static REGISTRATION_PAYLOAD_SCHEMA: OnceBox<Micheline> = OnceBox::new();
    REGISTRATION_PAYLOAD_SCHEMA.get_or_init(|| {
        let schema: Micheline = pair(vec![
            // allow_only_verified_sources
            bool_type(),
            // allowed_sources
            option(set(bytes())),
            // RegistrationExtra
            pair(vec![
                // instant_match
                option(
                    // PlannedExecutions
                    set(pair(vec![
                    // source
                    bytes(),
                    // start_delay
                    nat()
                ]))),
                // min_reputation
                option(nat()),
                // reward
                nat(),
                // slots
                nat(),
            ]),
            // job_id
            nat(),
            // memory
            nat(),
            // network_requests
            nat(),
            // required_modules
            set(nat()),
            // schedule
            pair(
                // Schedules
                vec![
                // duration
                nat(),
                // end_time
                nat(),
                // interval
                nat(),
                // max_start_delay
                nat(),
                // start_time
                nat(),
            ]),
            // script
            bytes(),
            // storage
            nat(),
        ]);
        Box::new(schema)
    })
}

/// The structure of a [`RawAction::SetJobEnvironment`] action before flattening:
///
/// ```txt
/// sp.TRecord(
///     job_id = sp.TNat,
///     processors = sp.TMap(sp.TBytes, sp.TMap(sp.TBytes, sp.TBytes))
///     public_key = sp.TBytes,
/// ).right_comb()
/// ```
fn set_job_environment_payload_schema() -> Micheline {
    pair(vec![
        // job_id
        nat(),
        // processors
        map(bytes(), map(bytes(), bytes())),
        // public_key
        bytes(),
    ])
}

/// The structure of a [`RawAction::DeregisterJob`] action before flattening:
///
/// ```txt
/// jobId=sp.TNat
/// ```
#[cfg_attr(rustfmt, rustfmt::skip)]
fn deregister_job_schema() -> &'static Micheline {
    static DEREGISTRATION_PAYLOAD_SCHEMA: OnceBox<Micheline> = OnceBox::new();
    DEREGISTRATION_PAYLOAD_SCHEMA.get_or_init(|| {
        let schema: Micheline = nat();
        Box::new(schema)
    })
}

/// The structure of a [`RawAction::FinalizeJob`] action before flattening:
///
/// ```txt
/// jobIds=sp.TSet(sp.TNat)
/// ```
#[cfg_attr(rustfmt, rustfmt::skip)]
fn finalize_job_schema() -> &'static Micheline {
    static DEREGISTRATION_PAYLOAD_SCHEMA: OnceBox<Micheline> = OnceBox::new();
    DEREGISTRATION_PAYLOAD_SCHEMA.get_or_init(|| {
        let schema: Micheline = set(nat());
        Box::new(schema)
    })
}

/// Parses an encoded [`RawAction::RegisterJob`] action's payload into [`JobRegistration`].
fn parse_job_registration_payload<
    Balance,
    ParsableAccountId,
    AccountId,
    MaxAllowedSources,
    MaxSlots,
    Extra,
>(
    encoded: &[u8],
) -> Result<
    (
        JobIdSequence,
        JobRegistration<AccountId, MaxAllowedSources, Extra>,
    ),
    TezosValidationError,
>
where
    ParsableAccountId: TryFrom<Vec<u8>> + Into<AccountId>,
    Extra: From<RegistrationExtra<Balance, AccountId, MaxSlots>>,
    Balance: From<u128>,
    MaxAllowedSources: ParameterBound,
    MaxSlots: ParameterBound,
{
    let unpacked: Micheline = Micheline::unpack(encoded, Some(registration_payload_schema()))
        .map_err(|e| TezosValidationError::TezosMicheline(e))?;

    let p: PrimitiveApplication = unpacked.try_into()?;
    let pair: Pair = p.try_into()?;

    let values = pair.flatten().values;
    let mut iter = values.into_iter();

    // !!! [IMPORTANT]: The values need to be decoded alphabetically !!!

    let allow_only_verified_sources: bool = try_bool(iter.next().ok_or(
        TezosValidationError::MissingField(FieldError::AllowOnlyVerifiedSources),
    )?)?;
    let allowed_sources = try_option(
        iter.next().ok_or(TezosValidationError::MissingField(
            FieldError::AllowedSources,
        ))?,
        |value| {
            let seq = try_sequence(value, |source| {
                let s: Vec<u8> = (&try_bytes::<_, Bytes, _>(source)?).into();
                let parsed: ParsableAccountId = s
                    .try_into()
                    .map_err(|_| TezosValidationError::AddressParsing)?;
                Ok(parsed.into())
            })?;
            Ok(AllowedSources::try_from(seq).map_err(|_| {
                TezosValidationError::LengthExceeded(LengthExceededError::AllowedSources)
            })?)
        },
    )?;
    let instant_match = try_option(
        iter.next()
            .ok_or(TezosValidationError::MissingField(FieldError::InstantMatch))?,
        |value| {
            let sources = try_sequence(value, |planned_execution| {
                let pair: Pair = planned_execution.try_into()?;
                let values = pair.flatten().values;
                if values.len() != 2 {
                    Err(TezosValidationError::InvalidMessage)?;
                }
                let mut iter = values.into_iter();

                let source = {
                    let s: Vec<u8> = (&try_bytes::<_, Bytes, _>(
                        iter.next()
                            .ok_or(TezosValidationError::MissingField(FieldError::Source))?,
                    )?)
                        .into();
                    let parsed: ParsableAccountId = s
                        .try_into()
                        .map_err(|_| TezosValidationError::AddressParsing)?;
                    Ok::<AccountId, TezosValidationError>(parsed.into())
                }?;

                let start_delay = {
                    let v: Int = try_int(
                        iter.next()
                            .ok_or(TezosValidationError::MissingField(FieldError::StartDelay))?,
                    )?;
                    v.to_integer()?
                };

                Ok(PlannedExecution {
                    source,
                    start_delay,
                })
            })?;

            Ok(PlannedExecutions::<AccountId, MaxSlots>::try_from(sources)
                .map_err(|_| TezosValidationError::InstantMatchPlannedExecutionsOutOfBounds)?)
        },
    )?;
    let min_reputation = try_option(
        iter.next().ok_or(TezosValidationError::MissingField(
            FieldError::MinReputation,
        ))?,
        |value| {
            let v: Int = try_int(value)?;
            Ok(v.to_integer()?)
        },
    )?;
    let reward = {
        let v: Int = try_int(
            iter.next()
                .ok_or(TezosValidationError::MissingField(FieldError::Reward))?,
        )?;
        let v: u128 = v.to_integer()?;
        v.into()
    };
    let slots = {
        let v: Int = try_int(
            iter.next()
                .ok_or(TezosValidationError::MissingField(FieldError::Slots))?,
        )?;
        v.to_integer()?
    };
    let job_id = {
        let v: Int = try_int(
            iter.next()
                .ok_or(TezosValidationError::MissingField(FieldError::JobId))?,
        )?;
        v.to_integer()?
    };
    let memory = {
        let v: Int = try_int(
            iter.next()
                .ok_or(TezosValidationError::MissingField(FieldError::Memory))?,
        )?;
        v.to_integer()?
    };
    let network_requests = {
        let v: Int = try_int(iter.next().ok_or(TezosValidationError::MissingField(
            FieldError::NetworkRequests,
        ))?)?;
        v.to_integer()?
    };

    let required_modules_unparsed = iter.next().ok_or(TezosValidationError::MissingField(
        FieldError::RequiredModules,
    ))?;
    let required_modules = try_sequence::<JobModule, _>(required_modules_unparsed, |module| {
        let value: Int = module.try_into()?;
        value
            .to_integer::<u32>()?
            .try_into()
            .map_err(|_| TezosValidationError::RequiredModulesParsing)
    })?
    .try_into()
    .map_err(|_| TezosValidationError::RequiredModulesParsing)?;

    let duration = {
        let v: Int = try_int(
            iter.next()
                .ok_or(TezosValidationError::MissingField(FieldError::Duration))?,
        )?;
        v.to_integer()?
    };
    let end_time = {
        let v: Int = try_int(
            iter.next()
                .ok_or(TezosValidationError::MissingField(FieldError::EndTime))?,
        )?;
        v.to_integer()?
    };
    let interval = {
        let v: Int = try_int(
            iter.next()
                .ok_or(TezosValidationError::MissingField(FieldError::Interval))?,
        )?;
        v.to_integer()?
    };
    let max_start_delay = {
        let v: Int = try_int(iter.next().ok_or(TezosValidationError::MissingField(
            FieldError::MaxStartDelay,
        ))?)?;
        v.to_integer()?
    };
    let start_time = {
        let v: Int = try_int(
            iter.next()
                .ok_or(TezosValidationError::MissingField(FieldError::StartTime))?,
        )?;
        v.to_integer()?
    };

    let script = {
        let script: Vec<u8> = (&try_bytes::<_, Bytes, _>(
            iter.next()
                .ok_or(TezosValidationError::MissingField(FieldError::Script))?,
        )?)
            .into();
        script
            .try_into()
            .map_err(|_| TezosValidationError::ScriptOutOfBounds)?
    };
    let storage = {
        let v: Int = try_int(
            iter.next()
                .ok_or(TezosValidationError::MissingField(FieldError::Storage))?,
        )?;
        v.to_integer()?
    };

    let extra: Extra = RegistrationExtra {
        requirements: JobRequirements {
            slots,
            reward,
            min_reputation,
            instant_match,
        },
    }
    .into();
    Ok((
        job_id,
        JobRegistration {
            script,
            allowed_sources,
            allow_only_verified_sources,
            schedule: Schedule {
                duration,
                start_time,
                end_time,
                interval,
                max_start_delay,
            },
            memory,
            network_requests,
            storage,
            required_modules,
            extra,
        },
    ))
}

/// Parses an encoded [`RawAction::SetJobEnvironment`] action's payload into [`SetJobEnvironment`].
fn parse_set_job_environment_payload<T: pallet_acurast::Config, ParsableAccountId>(
    encoded: &[u8],
) -> Result<
    (
        JobIdSequence,
        BoundedVec<(T::AccountId, EnvironmentFor<T>), T::MaxSlots>,
    ),
    TezosValidationError,
>
where
    ParsableAccountId: TryFrom<Vec<u8>> + Into<T::AccountId>,
{
    let unpacked: Micheline =
        Micheline::unpack(encoded, Some(&set_job_environment_payload_schema()))
            .map_err(|e| TezosValidationError::TezosMicheline(e))?;

    let p: PrimitiveApplication = unpacked.try_into()?;
    let pair: Pair = p.try_into()?;

    let values = pair.flatten().values;
    let mut iter = values.into_iter();

    let job_id = {
        let v: Int = try_int(
            iter.next()
                .ok_or(TezosValidationError::MissingField(FieldError::JobId))?,
        )?;
        v.to_integer()?
    };

    let processors: Vec<(
        T::AccountId,
        Vec<(
            BoundedVec<u8, T::EnvKeyMaxSize>,
            BoundedVec<u8, T::EnvValueMaxSize>,
        )>,
    )> = try_sequence::<
        (
            T::AccountId,
            Vec<(
                BoundedVec<u8, T::EnvKeyMaxSize>,
                BoundedVec<u8, T::EnvValueMaxSize>,
            )>,
        ),
        _,
    >(
        iter.next()
            .ok_or(TezosValidationError::MissingField(FieldError::Processors))?,
        |entry| {
            let element: Elt = entry.try_into()?;

            let source = {
                let source_bytes: Bytes = try_bytes::<_, Bytes, _>(*element.key)?;
                let source: Vec<u8> = (&source_bytes).into();
                let parsed: ParsableAccountId = source
                    .try_into()
                    .map_err(|_| TezosValidationError::AddressParsing)?;
                Ok::<T::AccountId, TezosValidationError>(parsed.into())
            }?;

            let variables: Vec<(
                BoundedVec<u8, T::EnvKeyMaxSize>,
                BoundedVec<u8, T::EnvValueMaxSize>,
            )> = try_sequence::<
                (
                    BoundedVec<u8, T::EnvKeyMaxSize>,
                    BoundedVec<u8, T::EnvValueMaxSize>,
                ),
                _,
            >(*element.value, |entry| {
                let element: Elt = entry.try_into()?;

                let variable_key_bytes: Bytes = try_bytes::<_, Bytes, _>(*element.key)?;
                let variable_key: Vec<u8> = (&variable_key_bytes).into();
                let variable_value_bytes: Bytes = try_bytes::<_, Bytes, _>(*element.value)?;
                let variable_value: Vec<u8> = (&variable_value_bytes).into();

                Ok((
                    BoundedVec::truncate_from(variable_key),
                    BoundedVec::truncate_from(variable_value),
                ))
            })?
            .try_into()
            .map_err(|_| TezosValidationError::ProcessorEnvironmentParsing)?;

            Ok((source, variables))
        },
    )?
    .try_into()
    .map_err(|_| TezosValidationError::ProcessorEnvironmentParsing)?;

    let public_key_bytes: Bytes = try_bytes::<_, Bytes, _>(
        iter.next()
            .ok_or(TezosValidationError::MissingField(FieldError::PublicKey))?,
    )?;
    let public_key: Vec<u8> = (&public_key_bytes).into();

    let env: Vec<(T::AccountId, EnvironmentFor<T>)> = processors
        .iter()
        .map(|el| {
            (
                el.0.clone(),
                Environment {
                    public_key: BoundedVec::truncate_from(public_key.clone()),
                    variables: BoundedVec::truncate_from(el.1.clone()),
                },
            )
        })
        .collect();

    Ok((job_id, BoundedVec::truncate_from(env)))
}

/// Parses an encoded [`RawAction::DeregisterJob`] action's payload into [`JobIdSequence`].
fn parse_deregister_job_payload(encoded: &[u8]) -> Result<JobIdSequence, TezosValidationError> {
    let unpacked: Micheline = Micheline::unpack(encoded, Some(deregister_job_schema()))
        .map_err(|e| TezosValidationError::TezosMicheline(e))?;

    let v: Int =
        try_nat(unpacked).map_err(|_| TezosValidationError::MissingField(FieldError::JobId))?;
    Ok(v.to_integer()?)
}

/// Parses an encoded [`RawAction::FinalizeJob`] action's payload into [[`JobIdSequence`]].
fn parse_finalize_job_payload(encoded: &[u8]) -> Result<Vec<JobIdSequence>, TezosValidationError> {
    let unpacked: Micheline = Micheline::unpack(encoded, Some(finalize_job_schema()))
        .map_err(|e| TezosValidationError::TezosMicheline(e))?;

    let ids = try_sequence::<JobIdSequence, _>(unpacked.try_into()?, |item| {
        let job_id_seq: Int =
            try_int(item).map_err(|_| TezosValidationError::MissingField(FieldError::JobId))?;
        job_id_seq
            .to_integer::<u32>()?
            .try_into()
            .map_err(|_| TezosValidationError::MissingField(FieldError::JobId))
    })?
    .try_into()
    .map_err(|_| TezosValidationError::MissingField(FieldError::JobId))?;

    Ok(ids)
}

fn bounded_address(
    address: &TezosAddress,
) -> Result<BoundedVec<u8, CU32<36>>, TezosValidationError> {
    let v: Vec<u8> = match &address {
        TezosAddress::Implicit(a) => a.try_into()?,
        TezosAddress::Originated(a) => a.try_into()?,
    };
    Ok(BoundedVec::<u8, CU32<36>>::try_from(v.to_owned())
        .map_err(|_| TezosValidationError::TezosAddressOutOfBounds)?)
}

/// Errors returned by this crate.
#[derive(RuntimeDebug, Display, From)]
#[cfg_attr(feature = "std", derive(DError))]
pub enum TezosValidationError {
    TezosMicheline(TezosMichelineError),
    TezosCore(TezosCoreError),
    ParsingFailure,
    InvalidMessage,
    InvalidAction,
    ScriptOutOfBounds,
    InstantMatchPlannedExecutionsOutOfBounds,
    TezosAddressOutOfBounds,
    InvalidReward,
    MissingField(FieldError),
    LengthExceeded(LengthExceededError),
    InvalidBool,
    InvalidOption,
    AddressParsing,
    RequiredModulesParsing,
    ProcessorEnvironmentParsing,
}

#[derive(RuntimeDebug, Display, From)]
#[cfg_attr(feature = "std", derive(DError))]
pub enum FieldError {
    ACTION,
    ORIGIN,
    PAYLOAD,
    AllowOnlyVerifiedSources,
    AllowedSources,
    Destination,
    InstantMatch,
    Source,
    StartDelay,
    MinReputation,
    Reward,
    Slots,
    JobId,
    Memory,
    NetworkRequests,
    Duration,
    EndTime,
    Interval,
    MaxStartDelay,
    RequiredModules,
    StartTime,
    Script,
    Storage,
    Processors,
    PublicKey,
}

#[derive(RuntimeDebug, Display, From)]
#[cfg_attr(feature = "std", derive(DError))]
pub enum LengthExceededError {
    AllowedSources,
}

/// Utility function to parse a tezos [`Bool`] into a Rust bool.
fn try_bool(value: Data) -> Result<bool, TezosValidationError> {
    match value {
        Data::True(_) => Ok(true),
        Data::False(_) => Ok(false),
        _ => Err(TezosValidationError::InvalidBool),
    }
}

/// Utility function to parse a tezos [`Bool`] into a Rust bool.
fn try_address(value: Data) -> Result<TezosAddress, TezosValidationError> {
    let origin: data::String = try_string(value)?;
    let origin: TezosAddress = origin.to_str().try_into()?;
    Ok(origin)
}

/// Utility function to parse a tezos [`MichelsonOption`] into a Rust Option, applying a conversion operation once to *Some* value.
fn try_option<R, O: FnOnce(Data) -> Result<R, TezosValidationError>>(
    value: Data,
    op: O,
) -> Result<Option<R>, TezosValidationError> {
    match value {
        Data::Some(v) => Ok(Some(op(*v.value)?)),
        Data::None(_) => Ok(None),
        _ => Err(TezosValidationError::InvalidOption),
    }
}

/// Utility function to parse a tezos [`Sequence`] into a [`Vec`], applying a conversion operation to each item of the sequence.
fn try_sequence<R, O: Fn(Data) -> Result<R, TezosValidationError>>(
    value: Data,
    op: O,
) -> Result<Vec<R>, TezosValidationError> {
    let s: Sequence = value.try_into()?;
    s.into_values().into_iter().map(|item| op(item)).collect()
}

/// Hashes `(owner, key, value)` to derive the leaf hash for the merkle proof.
pub fn leaf_hash<T: crate::pallet::Config<I>, I: 'static>(
    owner: StateOwner,
    key: StateKey,
    value: StateValue,
) -> H256 {
    let mut combined = vec![0_u8; owner.len() + key.len() + value.len()];
    combined[..owner.len()].copy_from_slice(&owner.as_ref());
    combined[owner.len()..owner.len() + key.len()].copy_from_slice(&key.as_ref());
    combined[owner.len() + key.len()..].copy_from_slice(&value.as_ref());
    T::TargetChainHashing::hash(&combined)
}

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, Eq, PartialEq)]
#[scale_info(skip_type_params(AccountConverter))]
pub struct TezosProof<AccountConverter, AccountId> {
    pub items: StateProof<H256>,
    pub path: StateKey,
    pub value: StateValue,
    #[cfg(any(test, feature = "runtime-benchmarks"))]
    pub marker: PhantomData<(AccountConverter, AccountId)>,
    #[cfg(not(any(test, feature = "runtime-benchmarks")))]
    marker: PhantomData<(AccountConverter, AccountId)>,
}

impl<T, I: 'static, AccountConverter> traits::Proof<T, I>
    for TezosProof<AccountConverter, T::AccountId>
where
    T: crate::pallet::Config<I>,
    T::RegistrationExtra: From<RegistrationExtra<T::Balance, T::AccountId, T::MaxSlots>>,
    AccountConverter: TryFrom<Vec<u8>> + Into<T::AccountId>,
{
    type Error = TezosValidationError;

    fn calculate_root(self: &Self) -> Result<[u8; 32], Self::Error> {
        let leaf_hash = leaf_hash::<T, I>(
            <CurrentTargetChainOwner<T, I>>::get(),
            self.path.clone(),
            self.value.clone(),
        );
        Ok(derive_proof::<T::TargetChainHashing, _>(self.items.clone(), leaf_hash).into())
    }

    fn message_id(self: &Self) -> Result<MessageIdentifier, Self::Error> {
        let schema = primitive_application(ComparableTypePrimitive::Nat).into();
        let micheline: Micheline = Micheline::unpack(self.path.as_ref(), Some(&schema))
            .map_err(|e| TezosValidationError::TezosMicheline(e))?;

        let value: Nat = micheline.try_into()?;
        value
            .to_integer()
            .map_err(|_| TezosValidationError::ParsingFailure)
    }

    fn message(self: &Self) -> Result<ParsedAction<T>, Self::Error> {
        let (action, origin, payload) = parse_message(&self.value)?;

        Ok(match action {
            RawAction::RegisterJob => {
                let payload: Vec<u8> = (&payload).into();
                let (job_id_sequence, registration) = parse_job_registration_payload::<
                    T::Balance,
                    AccountConverter,
                    T::AccountId,
                    T::MaxAllowedSources,
                    T::MaxSlots,
                    T::RegistrationExtra,
                >(payload.as_slice())?;

                ParsedAction::RegisterJob(
                    (
                        MultiOrigin::Tezos(bounded_address(&origin)?),
                        job_id_sequence,
                    ),
                    registration,
                )
            }
            RawAction::DeregisterJob => {
                let payload: Vec<u8> = (&payload).into();
                let job_id_sequence = parse_deregister_job_payload(payload.as_slice())?;

                ParsedAction::DeregisterJob((
                    MultiOrigin::Tezos(bounded_address(&origin)?),
                    job_id_sequence,
                ))
            }
            RawAction::FinalizeJob => {
                let payload: Vec<u8> = (&payload).into();
                let job_ids = parse_finalize_job_payload(payload.as_slice())?;

                let address = bounded_address(&origin)?;
                ParsedAction::FinalizeJob(
                    job_ids
                        .into_iter()
                        .map(|job_id_seq| (MultiOrigin::Tezos(address.clone()), job_id_seq))
                        .collect(),
                )
            }
            RawAction::SetJobEnvironment => {
                let payload: Vec<u8> = (&payload).into();
                let (job_id_sequence, set_job_environment) =
                    parse_set_job_environment_payload::<T, AccountConverter>(payload.as_slice())?;

                ParsedAction::SetJobEnvironment(
                    (
                        MultiOrigin::Tezos(bounded_address(&origin)?),
                        job_id_sequence,
                    ),
                    set_job_environment,
                )
            }
            RawAction::Noop => ParsedAction::Noop,
        })
    }
}

#[cfg(test)]
mod tests {
    use hex_literal::hex;
    use sp_runtime::bounded_vec;
    use tezos_core::types::encoded::ImplicitAddress;

    use pallet_acurast::{JobRegistration, Script};

    use crate::instances::TezosInstance;
    use crate::mock::*;
    use crate::Config;

    use super::*;

    #[test]
    fn test_unpack_register_job() -> Result<(), TezosValidationError> {
        let encoded = &hex!("050707010000000c52454749535445525f4a4f4207070a0000001601d1371b91fdbd07c8855659c84652230be0eaecd5000a000000e7050707030a0707050902000000250a000000200000000000000000000000000000000000000000000000000000000000000000070707070509020000002907070a000000201111111111111111111111111111111111111111111111111111111111111111000007070306070700a80f00010707000107070001070700010707020000000200000707070700b0d403070700bfe6d987d86107070098e4030707000000bf9a9f87d86107070a00000035697066733a2f2f516d64484c6942596174626e6150645573544d4d4746574534326353414a43485937426f374144583263644465610001");
        let (action, origin, payload) = parse_message(encoded)?;
        assert_eq!(RawAction::RegisterJob, action);
        let exp: TezosAddress = "KT1TezoooozzSmartPyzzSTATiCzzzwwBFA1".try_into().unwrap();
        assert_eq!(exp, origin);

        let payload: Vec<u8> = (&payload).into();
        let (job_id, registration): (
            JobIdSequence,
            JobRegistration<
                <Test as frame_system::Config>::AccountId,
                MaxAllowedSources,
                RegistrationExtra<
                    Balance,
                    <Test as frame_system::Config>::AccountId,
                    <Test as pallet_acurast::Config>::MaxSlots,
                >,
            >,
        ) = parse_job_registration_payload::<
            _,
            <Test as Config<TezosInstance>>::ParsableAccountId,
            <Test as frame_system::Config>::AccountId,
            <Test as pallet_acurast::Config>::MaxAllowedSources,
            <Test as pallet_acurast::Config>::MaxSlots,
            _,
        >(payload.as_slice())?;
        let expected = JobRegistration::<
            <Test as frame_system::Config>::AccountId,
            <Test as pallet_acurast::Config>::MaxAllowedSources,
            _,
        > {
            script: Script::try_from(vec![
                105, 112, 102, 115, 58, 47, 47, 81, 109, 100, 72, 76, 105, 66, 89, 97, 116, 98,
                110, 97, 80, 100, 85, 115, 84, 77, 77, 71, 70, 87, 69, 52, 50, 99, 83, 65, 74, 67,
                72, 89, 55, 66, 111, 55, 65, 68, 88, 50, 99, 100, 68, 101, 97,
            ])
            .unwrap(),
            allowed_sources: Some(bounded_vec![hex!(
                "0000000000000000000000000000000000000000000000000000000000000000"
            )
            .into()]),
            allow_only_verified_sources: true,
            schedule: Schedule {
                duration: 30000,
                start_time: 1678266066623,
                end_time: 1678266546623,
                interval: 31000,
                max_start_delay: 0,
            },
            memory: 1,
            network_requests: 1,
            storage: 1,
            required_modules: vec![JobModule::DataEncryption].try_into().unwrap(),
            extra: RegistrationExtra {
                requirements: JobRequirements {
                    slots: 1,
                    reward: 1000,
                    min_reputation: None,
                    instant_match: Some(bounded_vec![PlannedExecution {
                        source: hex![
                            "1111111111111111111111111111111111111111111111111111111111111111"
                        ]
                        .into(),
                        start_delay: 0,
                    }]),
                },
            },
        };

        assert_eq!(expected, registration);
        assert_eq!(1, job_id);
        Ok(())
    }

    #[test]
    fn test_unpack_register_job2() -> Result<(), TezosValidationError> {
        let encoded = &hex!("050707010000000c52454749535445525f4a4f4207070a0000001600008a8584be3718453e78923713a6966202b05f99c60a000000ed050707030a0707050902000000250a00000020d80a8b0d800a3320528693947f7317871b2d51e5f3c8f3d0d4e4f7e6938ed68f070707070509020000002907070a00000020d80a8b0d800a3320528693947f7317871b2d51e5f3c8f3d0d4e4f7e6938ed68f000007070509000007070080c0a8ca9a3a000107070001070700a40107070001070702000000000707070700b40707070080da9ce59b62070700a0cf24070700909c010080bbd3e49b6207070a00000035697066733a2f2f516d536e317252737a444b354258634e516d4e367543767a4d376858636548555569426b61777758396b534d474b0000");
        let (action, origin, payload) = parse_message(encoded)?;
        assert_eq!(RawAction::RegisterJob, action);
        let exp = TezosAddress::Implicit(ImplicitAddress::TZ1(
            "tz1YGTtd1hqGYTYKtcWSXYKSgCj5hvjaTPVd".try_into().unwrap(),
        ));
        assert_eq!(exp, origin);

        let payload: Vec<u8> = (&payload).into();
        let (job_id, registration): (
            JobIdSequence,
            JobRegistration<
                <Test as frame_system::Config>::AccountId,
                <Test as pallet_acurast::Config>::MaxAllowedSources,
                RegistrationExtra<
                    Balance,
                    <Test as frame_system::Config>::AccountId,
                    <Test as pallet_acurast::Config>::MaxSlots,
                >,
            >,
        ) = parse_job_registration_payload::<
            _,
            <Test as Config<TezosInstance>>::ParsableAccountId,
            <Test as frame_system::Config>::AccountId,
            <Test as pallet_acurast::Config>::MaxAllowedSources,
            <Test as pallet_acurast::Config>::MaxSlots,
            _,
        >(payload.as_slice())?;

        // JobRegistration { script: BoundedVec([105, 112, 102, 115, 58, 47, 47, 81, 109, 83, 110, 49, 114, 82, 115, 122, 68, 75, 53, 66, 88, 99, 78, 81, 109, 78, 54, 117, 67, 118, 122, 77, 55, 104, 88, 99, 101, 72, 85, 85, 105, 66, 107, 97, 119, 119, 88, 57, 107, 83, 77, 71, 75], 53), allowed_sources: Some([d80a8b0d800a3320528693947f7317871b2d51e5f3c8f3d0d4e4f7e6938ed68f (5GwyLDtS...)]), allow_only_verified_sources: true, schedule: Schedule { duration: 500, start_time: 1687356600000, end_time: 1687357200000, interval: 300000, max_start_delay: 10000 }, memory: 100, network_requests: 1, storage: 0, required_modules: BoundedVec([], 1), extra: RegistrationExtra { requirements: JobRequirements { slots: 1, reward: 1000000000000, min_reputation: Some(0), instant_match: Some([PlannedExecution { source: d80a8b0d800a3320528693947f7317871b2d51e5f3c8f3d0d4e4f7e6938ed68f (5GwyLDtS...), start_delay: 0 }]) } } }

        let expected = JobRegistration::<
            <Test as frame_system::Config>::AccountId,
            <Test as pallet_acurast::Config>::MaxAllowedSources,
            _,
        > {
            script: Script::try_from(vec![
                105, 112, 102, 115, 58, 47, 47, 81, 109, 83, 110, 49, 114, 82, 115, 122, 68, 75,
                53, 66, 88, 99, 78, 81, 109, 78, 54, 117, 67, 118, 122, 77, 55, 104, 88, 99, 101,
                72, 85, 85, 105, 66, 107, 97, 119, 119, 88, 57, 107, 83, 77, 71, 75,
            ])
            .unwrap(),
            allowed_sources: Some(bounded_vec![hex!(
                "d80a8b0d800a3320528693947f7317871b2d51e5f3c8f3d0d4e4f7e6938ed68f"
            )
            .into()]),
            allow_only_verified_sources: true,
            schedule: Schedule {
                duration: 500,
                start_time: 1687356600000,
                end_time: 1687357200000,
                interval: 300000,
                max_start_delay: 10000,
            },
            memory: 100,
            network_requests: 1,
            storage: 0,
            required_modules: vec![].try_into().unwrap(),
            extra: RegistrationExtra {
                requirements: JobRequirements {
                    slots: 1,
                    reward: 1000000000000,
                    min_reputation: Some(0),
                    instant_match: Some(bounded_vec![PlannedExecution {
                        source: hex![
                            "d80a8b0d800a3320528693947f7317871b2d51e5f3c8f3d0d4e4f7e6938ed68f"
                        ]
                        .into(),
                        start_delay: 0,
                    }]),
                },
            },
        };

        assert_eq!(expected, registration);
        assert_eq!(1, job_id);
        Ok(())
    }

    #[test]
    fn test_unpack_deregister_job() -> Result<(), TezosValidationError> {
        let encoded = &hex!("050707010000000e444552454749535445525f4a4f4207070a0000001600006b82198cb179e8306c1bedd08f12dc863f3288860a00000003050001");
        let (action, origin, payload) = parse_message(encoded)?;
        assert_eq!(RawAction::DeregisterJob, action);
        let exp: TezosAddress = "tz1VSUr8wwNhLAzempoch5d6hLRiTh8Cjcjb".try_into().unwrap();
        assert_eq!(exp, origin);

        let payload: Vec<u8> = (&payload).into();
        let job_id: JobIdSequence = parse_deregister_job_payload(payload.as_slice())?;

        assert_eq!(1, job_id);
        Ok(())
    }

    #[test]
    fn test_unpack_finalize_job() -> Result<(), TezosValidationError> {
        let encoded = &hex!("050707010000000c46494e414c495a455f4a4f4207070a0000001600008a8584be3718453e78923713a6966202b05f99c60a000000080502000000020001");
        let (action, origin, payload) = parse_message(encoded)?;
        assert_eq!(RawAction::FinalizeJob, action);
        let exp: TezosAddress = "tz1YGTtd1hqGYTYKtcWSXYKSgCj5hvjaTPVd".try_into().unwrap();
        assert_eq!(exp, origin);

        let payload: Vec<u8> = (&payload).into();
        let job_id: Vec<JobIdSequence> = parse_finalize_job_payload(payload.as_slice())?;

        assert_eq!(vec![1], job_id);
        Ok(())
    }

    #[test]
    fn test_unpack_set_job_environment() -> Result<(), TezosValidationError> {
        let encoded = &hex!("05070701000000135345545f4a4f425f454e5649524f4e4d454e5407070a0000001601d1371b91fdbd07c8855659c84652230be0eaecd5000a0000006e05070700010707020000003c07040a00000020d80a8b0d800a3320528693947f7317871b2d51e5f3c8f3d0d4e4f7e6938ed68f020000001007040a00000002abcd0a00000002abcd0a00000021028160f8d4230005bb3b6aa08078fe73b33b8db12d1d7b2083d593e585e64b061a");
        let (action, origin, payload) = parse_message(encoded)?;
        assert_eq!(RawAction::SetJobEnvironment, action);
        let exp: TezosAddress = "KT1TezoooozzSmartPyzzSTATiCzzzwwBFA1".try_into().unwrap();
        assert_eq!(exp, origin);

        let payload: Vec<u8> = (&payload).into();
        let (job_id, environment) = parse_set_job_environment_payload::<
            Test,
            <Test as Config<TezosInstance>>::ParsableAccountId,
        >(payload.as_slice())?;

        assert_eq!(job_id, 1);
        assert_eq!(environment.len(), 1);
        assert_eq!(
            environment[0].0,
            hex!("d80a8b0d800a3320528693947f7317871b2d51e5f3c8f3d0d4e4f7e6938ed68f").into()
        );
        assert_eq!(
            environment[0].1,
            Environment {
                public_key: BoundedVec::truncate_from(
                    hex!("028160f8d4230005bb3b6aa08078fe73b33b8db12d1d7b2083d593e585e64b061a")
                        .to_vec()
                ),
                variables: BoundedVec::truncate_from(vec![(
                    BoundedVec::truncate_from(hex!("abcd").to_vec()),
                    BoundedVec::truncate_from(hex!("abcd").to_vec())
                )])
            }
        );

        Ok(())
    }
}
