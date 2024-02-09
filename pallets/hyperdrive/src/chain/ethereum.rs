use super::util::evm;
use crate::{traits, MessageIdentifier, ParsedAction, RawAction};
use alloy_sol_types::{sol, SolType};
use codec::{Decode, Encode};
use core::marker::PhantomData;
use derive_more::{Display, From};
use frame_support::pallet_prelude::ConstU32;
use frame_support::BoundedVec;
use pallet_acurast::{
    AllowedSources, EthereumAddressBytes, JobModule, JobModules, JobRegistration, MultiOrigin,
    Schedule, Script,
};
use pallet_acurast_marketplace::{
    JobRequirements, PlannedExecution, PlannedExecutions, RegistrationExtra,
};
use rlp::Rlp;
use scale_info::TypeInfo;
use sp_core::Hasher;
use sp_runtime::traits::Keccak256;
use sp_runtime::RuntimeDebug;
use sp_std::vec::Vec;

const STORAGE_INDEX: u8 = 7u8;

// Declare a solidity type in standard solidity
sol! {
    struct Message {
        uint16 action;
        address origin;
        bytes payload;
    }

    type JobId is uint128;

    struct EthJobMatch {
        bytes32 source;
        uint64 startDelay;
    }

    struct EthJobRequirements {
        uint8 slots;
        uint128 reward;
        uint128 minReputation;
        EthJobMatch[] instantMatch;
    }

    struct EthJobSchedule {
        uint64 duration;
        uint64 startTime;
        uint64 endTime;
        uint64 interval;
        uint64 maxStartDelay;
    }

    struct AcurastJobRegistration {
        uint128 jobId;
        bytes32[] allowedSources;
        bool allowOnlyVerifiedSources;
        EthJobRequirements requirements;
        uint16[] requiredModules;
        bytes script;
        EthJobSchedule schedule;
        uint32 memoryCapacity;
        uint32 networkRequests;
        uint32 storageCapacity;
    }

    struct EthEnvironmentVariable {
        bytes key;
        bytes value;
    }

    struct EthProcessorEnvironmentVariables {
        bytes32 source;
        EthEnvironmentVariable[] variables;
    }

    struct EthEnvironmentVariablesPayload {
        uint128 jobId;
        bytes publicKey;
        EthProcessorEnvironmentVariables[] processors;
    }
}

/// Errors specific to the Ethereum instance
#[derive(RuntimeDebug, Display, From)]
pub enum EthereumValidationError {
    InvalidAccountProof,
    InvalidStorageProof,
    UnknownAction(u16),
    IllFormattedMessage,
    IllFormattedJobRegistration,
    IllFormattedEnvironmentVariablesPayload,
    InvalidOriginAddress,
    InvalidJobModule,
    CouldNotParseAcurastAddress,
    CouldNotDecodeRegisterJobPayload,
    CouldNotDecodeDeregisterJobPayload,
    CouldNotDecodeFinalizeJobPayload,
    TooManyPlannedExecutions,
    TooManyAllowedSources,
    TooManyJobModules,
    InvalidRlpEncoding,
}

pub type EthereumProofItem = BoundedVec<u8, ConstU32<1024>>;
pub type EthereumProofItems = BoundedVec<EthereumProofItem, ConstU32<32>>;
pub type EthereumProofValue = BoundedVec<u8, ConstU32<1024>>;

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, Eq, PartialEq)]
#[scale_info(skip_type_params(T, AccountConverter))]
pub struct EthereumProof<T, AccountConverter> {
    pub account_proof: EthereumProofItems,
    pub storage_proof: EthereumProofItems,
    pub message_id: MessageIdentifier,
    pub value: EthereumProofValue,
    #[cfg(any(test, feature = "runtime-benchmarks"))]
    pub marker: PhantomData<(T, AccountConverter)>,
    #[cfg(not(any(test, feature = "runtime-benchmarks")))]
    marker: PhantomData<(T, AccountConverter)>,
}

impl<T, I: 'static, AccountConverter> traits::Proof<T, I> for EthereumProof<T, AccountConverter>
where
    T: crate::pallet::Config<I>,
    T::RegistrationExtra: From<RegistrationExtra<T::Balance, T::AccountId, T::MaxSlots>>,
    AccountConverter: TryFrom<Vec<u8>> + Into<T::AccountId>,
{
    type Error = EthereumValidationError;

    fn calculate_root(self: &Self) -> Result<[u8; 32], Self::Error> {
        let account_proof: Vec<Vec<u8>> = self
            .account_proof
            .iter()
            .map(|node| node.to_vec())
            .collect();

        let storage_proof: Vec<Vec<u8>> = self
            .storage_proof
            .iter()
            .map(|node| node.to_vec())
            .collect();

        // Validate account proof
        let storage_owner_address = crate::pallet::Pallet::<T, I>::current_target_chain_owner();

        // Validate the storage proof against the known
        let account_path = Keccak256::hash(storage_owner_address.as_ref())
            .as_bytes()
            .to_vec();
        let storage_path = &evm::storage_path(&STORAGE_INDEX, &self.message_id).to_vec();
        let verified_value = evm::validate_storage_proof(
            &account_path,
            &storage_path,
            &account_proof,
            &storage_proof,
        )?;

        // Ensure the value extracted from the proof is equal to the hash of the message
        let message_hash: [u8; 32] = Keccak256::hash(&self.value).0;
        let verified_message_hash: &[u8] = Rlp::new(&verified_value).data().map_err(|err| {
            log::debug!("Could not decode rlp value: {:?}", err);
            #[cfg(test)]
            dbg!(err);

            EthereumValidationError::InvalidRlpEncoding
        })?;
        if verified_message_hash.ne(&message_hash) {
            return Err(EthereumValidationError::InvalidStorageProof);
        }

        // TODO: Temporary work around
        let root_hash = Keccak256::hash(&account_proof[0]);

        Ok(root_hash.0)
    }

    fn message_id(self: &Self) -> Result<MessageIdentifier, Self::Error> {
        Ok(self.message_id)
    }

    fn message(self: &Self) -> Result<ParsedAction<T>, Self::Error> {
        // EVM storage is divided in slots of 32 bytes. If the data is longer than 32 bytes,
        // the first slot will contain the length of the data.
        let value = if self.value.len() > 32 {
            &self.value[32..]
        } else {
            &self.value
        };
        let decoded: Message = Message::decode(value, false)
            .map_err(|_| EthereumValidationError::IllFormattedMessage)?;

        // Convert action index to its RawAction variant
        let action = RawAction::try_from(decoded.action)
            .map_err(|_| EthereumValidationError::UnknownAction(decoded.action))?;

        // Convert origin address to a multi origin
        let origin_address = EthereumAddressBytes::try_from(decoded.origin.to_vec())
            .map_err(|_| EthereumValidationError::InvalidOriginAddress)?;
        let origin = MultiOrigin::Ethereum(origin_address.clone());

        fn convert_account_id<Account, AccountConverter: TryFrom<Vec<u8>> + Into<Account>>(
            bytes: Vec<u8>,
        ) -> Result<Account, EthereumValidationError> {
            let parsed: AccountConverter = bytes
                .try_into()
                .map_err(|_| EthereumValidationError::CouldNotParseAcurastAddress)?;
            Ok(parsed.into())
        }

        match action {
            RawAction::RegisterJob => {
                let job_registration: AcurastJobRegistration =
                    AcurastJobRegistration::decode_single(&decoded.payload, true)
                        .map_err(|_| EthereumValidationError::IllFormattedJobRegistration)?;

                let job_id = (origin, job_registration.jobId.clone());

                fn convert_job_match<
                    AccountId,
                    AccountConverter: TryFrom<Vec<u8>> + Into<AccountId>,
                >(
                    m: EthJobMatch,
                ) -> Result<PlannedExecution<AccountId>, EthereumValidationError> {
                    Ok(PlannedExecution::<AccountId> {
                        source: convert_account_id::<AccountId, AccountConverter>(
                            m.source.to_vec(),
                        )?,
                        start_delay: m.startDelay,
                    })
                }
                let executions: PlannedExecutions<T::AccountId, T::MaxSlots> =
                    PlannedExecutions::try_from(
                        job_registration
                            .requirements
                            .instantMatch
                            .into_iter()
                            .map(convert_job_match::<T::AccountId, AccountConverter>)
                            .collect::<Result<Vec<_>, Self::Error>>()?,
                    )
                    .map_err(|_| EthereumValidationError::TooManyPlannedExecutions)?;

                let extra: T::RegistrationExtra = RegistrationExtra {
                    requirements: JobRequirements {
                        slots: job_registration.requirements.slots.into(),
                        reward: T::Balance::from(job_registration.requirements.reward),
                        min_reputation: Some(job_registration.requirements.minReputation),
                        instant_match: Some(executions),
                    },
                }
                .into();
                let allowed_sources: AllowedSources<T::AccountId, T::MaxAllowedSources> =
                    AllowedSources::try_from(
                        job_registration
                            .allowedSources
                            .into_iter()
                            .map(|item| {
                                convert_account_id::<T::AccountId, AccountConverter>(item.to_vec())
                            })
                            .collect::<Result<Vec<_>, Self::Error>>()?,
                    )
                    .map_err(|_| EthereumValidationError::TooManyAllowedSources)?;
                let required_modules: JobModules = JobModules::try_from(
                    job_registration
                        .requiredModules
                        .iter()
                        .map(|item| {
                            Ok(JobModule::try_from(*item as u32)
                                .map_err(|_| EthereumValidationError::InvalidJobModule)?)
                        })
                        .collect::<Result<Vec<_>, Self::Error>>()?,
                )
                .map_err(|_| EthereumValidationError::TooManyJobModules)?;
                let registration = JobRegistration {
                    script: Script::truncate_from(job_registration.script),
                    allowed_sources: Some(allowed_sources),
                    allow_only_verified_sources: job_registration.allowOnlyVerifiedSources,
                    schedule: Schedule {
                        duration: job_registration.schedule.duration,
                        start_time: job_registration.schedule.startTime,
                        end_time: job_registration.schedule.endTime,
                        interval: job_registration.schedule.interval,
                        max_start_delay: job_registration.schedule.maxStartDelay,
                    },
                    memory: job_registration.memoryCapacity,
                    network_requests: job_registration.networkRequests,
                    storage: job_registration.storageCapacity,
                    required_modules,
                    extra,
                };

                Ok(ParsedAction::RegisterJob(job_id, registration))
            }
            RawAction::DeregisterJob => {
                let job_id = JobId::decode_single(&decoded.payload, true)
                    .map_err(|_| EthereumValidationError::CouldNotDecodeDeregisterJobPayload)?;

                Ok(ParsedAction::DeregisterJob((origin, job_id)))
            }
            RawAction::FinalizeJob => {
                let jobs = <sol!(uint128[])>::decode_single(&decoded.payload, true)
                    .map_err(|_| EthereumValidationError::CouldNotDecodeFinalizeJobPayload)?
                    .iter()
                    .map(|id| (MultiOrigin::Ethereum(origin_address.clone()), *id))
                    .collect();

                Ok(ParsedAction::FinalizeJob(jobs))
            }
            RawAction::SetJobEnvironment => {
                let set_job_environments: EthEnvironmentVariablesPayload =
                    EthEnvironmentVariablesPayload::decode_single(&decoded.payload, true).map_err(
                        |_| EthereumValidationError::IllFormattedEnvironmentVariablesPayload,
                    )?;

                let job_id = (
                    MultiOrigin::Ethereum(origin_address.clone()),
                    set_job_environments.jobId,
                );

                let public_key = BoundedVec::truncate_from(set_job_environments.publicKey);

                let variables = set_job_environments
                    .processors
                    .iter()
                    .map(|entry| {
                        let processor = convert_account_id::<T::AccountId, AccountConverter>(
                            entry.source.to_vec(),
                        )?;

                        Ok((
                            processor,
                            pallet_acurast::Environment {
                                public_key: public_key.clone(),
                                variables: BoundedVec::truncate_from(
                                    entry
                                        .variables
                                        .iter()
                                        .map(|v| {
                                            let key = BoundedVec::truncate_from(v.key.clone());
                                            let value = BoundedVec::truncate_from(v.value.clone());

                                            (key, value)
                                        })
                                        .collect(),
                                ),
                            },
                        ))
                    })
                    .collect::<Result<Vec<_>, Self::Error>>()?;

                Ok(ParsedAction::SetJobEnvironment(
                    job_id,
                    BoundedVec::truncate_from(variables),
                ))
            }
            RawAction::Noop => Ok(ParsedAction::Noop),
        }
    }
}
