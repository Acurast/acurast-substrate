#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod proxy {
    use ink::{
        codegen::EmitEvent,
        env::{
            call::{build_call, ExecutionInput},
            hash, DefaultEnvironment,
        },
        prelude::{format, string::String, string::ToString, vec::Vec},
        storage::{traits::StorageLayout, Mapping},
        LangError,
    };
    use scale::{Decode, Encode};

    use acurast_core_ink::types::{
        IncomingAction, IncomingActionPayloadV1, OutgoingActionPayloadV1, RegisterJobMatchV1,
        RegisterJobPayloadV1, SetJobEnvironmentPayloadV1, SetProcessorJobEnvironmentV1, Version,
        VersionedIncomingActionPayload,
    };
    use acurast_validator_ink::validator::{LeafProof, MerkleProof};

    pub type OuterError<T> = Result<Result<T, ink::LangError>, ink::env::Error>;

    #[derive(Clone, Eq, PartialEq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct SetJobEnvironmentProcessor {
        pub address: AccountId,
        pub variables: Vec<(Vec<u8>, Vec<u8>)>,
    }

    #[derive(Clone, Eq, PartialEq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct UserPayloadSetJobEnvironmentAction {
        pub job_id: u128,
        pub public_key: Vec<u8>,
        pub processors: Vec<SetJobEnvironmentProcessor>,
    }

    #[derive(Clone, Eq, PartialEq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct RegisterJobMatch {
        pub source: AccountId,
        pub start_delay: u64,
    }

    #[derive(Clone, Eq, PartialEq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct UserPayloadRegisterJob {
        allowed_sources: Vec<AccountId>,
        allow_only_verified_sources: bool,
        destination: AccountId,
        required_modules: Vec<u16>,
        script: Vec<u8>,
        duration: u64,
        start_time: u64,
        end_time: u64,
        interval: u64,
        max_start_delay: u64,
        memory: u32,
        network_requests: u32,
        storage: u32,
        // Extra,
        slots: u8,
        reward: u128,
        min_reputation: Option<u128>,
        instant_match: Vec<RegisterJobMatch>,
        expected_fulfillment_fee: u128,
    }

    #[derive(Clone, Eq, PartialEq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum UserAction {
        RegisterJob(UserPayloadRegisterJob),
        DeregisterJob(u128),
        FinalizeJob(Vec<u128>),
        SetJobEnvironment(UserPayloadSetJobEnvironmentAction),
        Noop,
    }

    #[derive(Clone, Eq, PartialEq, Encode, Decode)]
    pub struct RawOutgoingAction {
        pub id: u64,
        pub origin: AccountId,
        pub payload_version: u16,
        pub payload: Vec<u8>,
    }

    #[derive(Clone, Eq, PartialEq, Decode)]
    pub struct RawIncomingAction {
        id: u64,
        payload_version: u16,
        payload: Vec<u8>,
    }

    fn decode_incoming_action(payload: &Vec<u8>) -> Result<IncomingAction, Error> {
        match IncomingAction::decode(&mut payload.as_slice()) {
            Err(err) => Err(Error::InvalidIncomingAction(format!("{:?}", err))),
            Ok(action) => Ok(action),
        }
    }

    #[derive(Clone, Eq, PartialEq, Encode, Decode)]
    pub enum StatusKind {
        /// Status after a job got registered.
        Open = 0,
        /// Status after a valid match for a job got submitted.
        Matched = 1,
        /// Status after all processors have acknowledged the job.
        Assigned = 2,
        /// Status when a job has been finalized or cancelled
        FinalizedOrCancelled = 3,
    }

    #[derive(Clone, Eq, PartialEq, Encode, Decode)]
    pub struct JobInformationV1 {
        creator: AccountId,
        destination: AccountId,
        processors: Vec<AccountId>,
        expected_fulfillment_fee: u128,
        remaining_fee: u128,
        maximum_reward: u128,
        slots: u8,
        status: StatusKind,
        start_time: u64,
        end_time: u64,
        interval: u64,
        abstract_data: Vec<u8>, // Abstract data, this field can be used to add new parameters to the job information structure after the contract has been deployed.
    }

    #[derive(Clone, Eq, PartialEq, Encode, Decode)]
    pub enum JobInformation {
        V1(JobInformationV1),
    }

    impl JobInformation {
        fn decode(instance: &Proxy, job_id: u128) -> Result<Self, Error> {
            match instance.get_job(job_id)? {
                (Version::V1, job_bytes) => {
                    let job =
                        JobInformationV1::decode(&mut job_bytes.as_slice()).map_err(|err| {
                            Error::Verbose(format!("Cannot decode job information V1 {:?}", err))
                        })?;

                    Ok(Self::V1(job))
                }
            }
        }
    }

    #[derive(Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum ConfigureArgument {
        SetOwner(AccountId),
        SetMerkleAggregator(AccountId),
        SetProofValidator(AccountId),
        SetPaused(bool),
        SetPayloadVersion(u16),
        SetJobInfoVersion(u16),
        SetMaxMessageBytes(u16),
        SetExchangeRatio(ExchangeRatio),
        SetCode([u8; 32]),
    }

    #[ink(event)]
    pub struct IncomingActionProcessed {
        action_id: u64,
    }

    /// Errors returned by the contract's methods.
    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        UnknownJobVersion(u16),
        UnknownIncomingActionVersion(u16),
        JobAlreadyFinished,
        NotJobProcessor,
        UnknownJob,
        InvalidProof,
        ContractPaused,
        NotOwner,
        NotJobCreator,
        CannotFinalizeJob,
        OutgoingActionTooBig,
        Verbose(String),
        UnknownActionIndex(u64),
        InvalidIncomingAction(String),
        InvalidOutgoingAction(String),
        /// Error wrappers
        StateAggregatorError(acurast_state_ink::Error),
        ValidatorError(acurast_validator_ink::Error),
        ConsumerError(String),
        LangError(LangError),
    }

    #[derive(Debug, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
    pub struct ExchangeRatio {
        pub numerator: u16,
        pub denominator: u16,
    }

    impl ExchangeRatio {
        fn exchange_price(&self, expected_acurast_amount: u128) -> u128 {
            // Calculate how many azero is required to cover for the job cost
            let amount =
                ((self.numerator as u128) * expected_acurast_amount) / (self.denominator as u128);

            if ((self.numerator as u128) * expected_acurast_amount) / (self.denominator as u128)
                != 0
            {
                amount + 1
            } else {
                amount
            }
        }
    }

    /// Contract configurations are contained in this structure
    #[ink::storage_item]
    #[derive(Debug)]
    pub struct Config {
        /// Address allowed to manage the contract
        owner: AccountId,
        /// The state aggregator
        merkle_aggregator: AccountId,
        /// The Merkle Mountain Range proof validator
        proof_validator: AccountId,
        /// Flag that states if the contract is paused or not
        paused: bool,
        /// Payload versioning
        payload_version: u16,
        /// Job information versioning
        job_info_version: u16,
        /// Maximum size per action
        max_message_bytes: u16,
        /// Exchange ratio ( AZERO / ACU )
        exchange_ratio: ExchangeRatio,
    }

    #[ink(storage)]
    pub struct Proxy {
        config: Config,
        next_outgoing_action_id: u64,
        processed_incoming_actions: Mapping<u64, ()>,
        next_job_id: u128,
        actions: Mapping<u64, (u64, u128, Vec<u8>)>,
        job_info: Mapping<u128, (u16, Vec<u8>)>,
    }

    impl Proxy {
        #[ink(constructor)]
        pub fn new(owner: AccountId, state: AccountId, validator: AccountId) -> Self {
            let mut contract = Self::default();

            contract.config.owner = owner;
            contract.config.merkle_aggregator = state;
            contract.config.proof_validator = validator;
            contract
        }

        #[ink(constructor)]
        pub fn default() -> Self {
            Self {
                config: Config {
                    owner: AccountId::from([0x0; 32]),
                    merkle_aggregator: AccountId::from([0x0; 32]),
                    proof_validator: AccountId::from([0x0; 32]),
                    paused: false,
                    payload_version: 1,
                    job_info_version: 1,
                    max_message_bytes: 2048,
                    exchange_ratio: ExchangeRatio {
                        numerator: 1,
                        denominator: 10,
                    },
                },
                next_outgoing_action_id: 1,
                processed_incoming_actions: Mapping::new(),
                next_job_id: 1,
                actions: Mapping::new(),
                job_info: Mapping::new(),
            }
        }

        fn fail_if_not_owner(&self) -> Result<(), Error> {
            if self.config.owner.eq(&self.env().caller()) {
                Ok(())
            } else {
                Err(Error::NotOwner)
            }
        }

        fn fail_if_paused(&self) -> Result<(), Error> {
            if self.config.paused {
                Err(Error::ContractPaused)
            } else {
                Ok(())
            }
        }

        fn blake2b_hash(data: &Vec<u8>) -> [u8; 32] {
            let mut output = <hash::Blake2x256 as hash::HashOutput>::Type::default();
            ink::env::hash_bytes::<hash::Blake2x256>(&data, &mut output);

            output
        }

        fn get_job(&self, job_id: u128) -> Result<(Version, Vec<u8>), Error> {
            if let Some((version, job_bytes)) = self.job_info.get(job_id) {
                match version {
                    o if o == Version::V1 as u16 => Ok((Version::V1, job_bytes)),
                    v => Err(Error::UnknownJobVersion(v)),
                }
            } else {
                Err(Error::UnknownJob)
            }
        }

        /// Modifies the code which is used to execute calls to this contract.
        pub fn set_code(&mut self, code_hash: [u8; 32]) {
            ink::env::set_code_hash(&code_hash).unwrap_or_else(|err| {
                panic!(
                    "Failed to `set_code_hash` to {:?} due to {:?}",
                    code_hash, err
                )
            });
            ink::env::debug_println!("Switched code hash to {:?}.", code_hash);
        }

        #[ink(message)]
        pub fn configure(&mut self, actions: Vec<ConfigureArgument>) -> Result<(), Error> {
            self.fail_if_not_owner()?;

            for action in actions {
                match action {
                    ConfigureArgument::SetOwner(address) => self.config.owner = address,
                    ConfigureArgument::SetMerkleAggregator(address) => {
                        self.config.merkle_aggregator = address
                    }
                    ConfigureArgument::SetProofValidator(address) => {
                        self.config.proof_validator = address
                    }
                    ConfigureArgument::SetPaused(paused) => self.config.paused = paused,
                    ConfigureArgument::SetPayloadVersion(version) => {
                        self.config.payload_version = version
                    }
                    ConfigureArgument::SetJobInfoVersion(version) => {
                        self.config.job_info_version = version
                    }
                    ConfigureArgument::SetMaxMessageBytes(max_size) => {
                        self.config.max_message_bytes = max_size
                    }

                    ConfigureArgument::SetExchangeRatio(ratio) => {
                        self.config.exchange_ratio = ratio
                    }
                    ConfigureArgument::SetCode(code_hash) => self.set_code(code_hash),
                }
            }

            Ok(())
        }

        /// This method is called by users to interact with the acurast protocol
        #[ink(message)]
        pub fn send_actions(&mut self, actions: Vec<UserAction>) -> Result<(), Error> {
            // The contract should not be paused
            self.fail_if_paused()?;

            let caller = self.env().caller();

            for action in actions {
                let outgoing_action = match action {
                    UserAction::RegisterJob(payload) => {
                        // Increment job identifier
                        let job_id = self.next_job_id;
                        self.next_job_id += 1;

                        // Calculate the number of executions that fit the job schedule
                        let start_time = payload.start_time;
                        let end_time = payload.end_time;
                        let interval = payload.interval;
                        if interval == 0 {
                            return Err(Error::Verbose("INTERVAL_CANNNOT_BE_ZERO".to_string()));
                        }
                        let execution_count = (end_time - start_time) / interval;

                        // Calculate the fee required for all job executions
                        let slots = payload.slots;
                        let expected_fulfillment_fee = payload.expected_fulfillment_fee;
                        let expected_fee =
                            ((slots as u128) * execution_count as u128) * expected_fulfillment_fee;

                        // Calculate the total reward required to pay all executions
                        let reward_per_execution = payload.reward;
                        let maximum_reward =
                            (slots as u128) * (execution_count as u128) * reward_per_execution;

                        // Get exchange price
                        let cost: u128 = self.config.exchange_ratio.exchange_price(maximum_reward);

                        // Validate job registration payment
                        if self.env().transferred_value() != expected_fee + cost {
                            return Err(Error::Verbose(
                                "AMOUNT_CANNOT_COVER_JOB_COSTS".to_string(),
                            ));
                        }

                        let info = JobInformationV1 {
                            creator: self.env().caller(),
                            destination: payload.destination,
                            processors: Vec::new(),
                            expected_fulfillment_fee,
                            remaining_fee: expected_fee,
                            maximum_reward,
                            slots,
                            status: StatusKind::Open,
                            start_time,
                            end_time,
                            interval,
                            abstract_data: Vec::new(),
                        };

                        self.job_info
                            .insert(self.next_job_id, &(Version::V1 as u16, info.encode()));

                        OutgoingActionPayloadV1::RegisterJob(RegisterJobPayloadV1 {
                            job_id,
                            allowed_sources: payload
                                .allowed_sources
                                .iter()
                                .map(|source| *source.as_ref())
                                .collect(),
                            allow_only_verified_sources: payload.allow_only_verified_sources,
                            destination: *payload.destination.as_ref(),
                            required_modules: payload.required_modules,
                            script: payload.script,
                            duration: payload.duration,
                            start_time: payload.start_time,
                            end_time: payload.end_time,
                            interval: payload.interval,
                            max_start_delay: payload.max_start_delay,
                            memory: payload.memory,
                            network_requests: payload.network_requests,
                            storage: payload.storage,
                            // Extra
                            slots: payload.slots,
                            reward: payload.reward,
                            min_reputation: payload.min_reputation,
                            instant_match: payload
                                .instant_match
                                .iter()
                                .map(|m| RegisterJobMatchV1 {
                                    source: *m.source.as_ref(),
                                    start_delay: m.start_delay,
                                })
                                .collect(),
                            expected_fulfillment_fee: payload.expected_fulfillment_fee,
                        })
                    }
                    UserAction::DeregisterJob(job_id) => {
                        match JobInformation::decode(self, job_id)? {
                            JobInformation::V1(job) => {
                                // Only the job creator can deregister the job
                                if job.creator != self.env().caller() {
                                    return Err(Error::NotJobCreator);
                                }
                            }
                        }
                        OutgoingActionPayloadV1::DeregisterJob(job_id)
                    }
                    UserAction::FinalizeJob(ids) => {
                        for id in ids.clone() {
                            match JobInformation::decode(self, id)? {
                                JobInformation::V1(job) => {
                                    // Only the job creator can finalize the job
                                    if job.creator != self.env().caller() {
                                        return Err(Error::NotJobCreator);
                                    }

                                    // Verify if job can be finalized
                                    let is_expired =
                                        (job.end_time / 1000) < self.env().block_timestamp().into();
                                    if !is_expired {
                                        return Err(Error::CannotFinalizeJob);
                                    }
                                }
                            }
                        }

                        OutgoingActionPayloadV1::FinalizeJob(ids)
                    }
                    UserAction::SetJobEnvironment(payload) => {
                        match JobInformation::decode(self, payload.job_id)? {
                            JobInformation::V1(job) => {
                                // Only the job creator can set environment variables
                                if job.creator != self.env().caller() {
                                    return Err(Error::NotJobCreator);
                                }
                            }
                        }
                        OutgoingActionPayloadV1::SetJobEnvironment(SetJobEnvironmentPayloadV1 {
                            job_id: payload.job_id,
                            public_key: payload.public_key,
                            processors: payload
                                .processors
                                .iter()
                                .map(|processor| SetProcessorJobEnvironmentV1 {
                                    address: *processor.address.as_ref(),
                                    variables: processor.variables.clone(),
                                })
                                .collect(),
                        })
                    }
                    UserAction::Noop => OutgoingActionPayloadV1::Noop,
                };

                let encoded_action = RawOutgoingAction {
                    id: self.next_outgoing_action_id,
                    origin: caller,
                    payload_version: self.config.payload_version,
                    payload: outgoing_action.encode(),
                }
                .encode();

                // Verify that the encoded action size is less than `max_message_bytes`
                if !encoded_action
                    .len()
                    .lt(&(self.config.max_message_bytes as usize))
                {
                    return Err(Error::OutgoingActionTooBig);
                }

                let call_result: OuterError<acurast_state_ink::InsertReturn> =
                    build_call::<DefaultEnvironment>()
                        .call(self.config.merkle_aggregator)
                        .exec_input(
                            ExecutionInput::new(acurast_state_ink::INSERT_SELECTOR)
                                .push_arg(Self::blake2b_hash(&encoded_action)),
                        )
                        .transferred_value(0)
                        .returns()
                        .try_invoke();

                match call_result {
                    // Errors from the underlying execution environment (e.g the Contracts pallet)
                    Err(error) => Err(Error::Verbose(format!("{:?}", error))),
                    // Errors from the programming language
                    Ok(Err(error)) => Err(Error::LangError(error)),
                    // Errors emitted by the contract being called
                    Ok(Ok(Err(error))) => Err(Error::StateAggregatorError(error)),
                    // Successful call result
                    Ok(Ok(Ok((leaf_index, snapshot)))) => {
                        // Store encoded action
                        self.actions.insert(
                            self.next_outgoing_action_id,
                            &(leaf_index, snapshot, encoded_action),
                        );

                        // Increment action id
                        self.next_outgoing_action_id += 1;

                        Ok(())
                    }
                }?;
            }

            Ok(())
        }

        /// This method purpose is to receive provable messages from the acurast protocol
        #[ink(message)]
        pub fn receive_actions(
            &mut self,
            snapshot: u128,
            proof: MerkleProof<[u8; 32]>,
        ) -> Result<(), Error> {
            // The contract cannot be paused
            self.fail_if_paused()?;

            let mut actions: Vec<IncomingAction> = proof
                .leaves
                .iter()
                .map(|leaf| decode_incoming_action(&leaf.data))
                .collect::<Result<Vec<IncomingAction>, Error>>()?;

            // Sort actions
            actions.sort();

            // Validate proof
            let call_result: OuterError<acurast_validator_ink::VerifyProofReturn> =
                build_call::<DefaultEnvironment>()
                    .call(self.config.proof_validator)
                    .exec_input(
                        ExecutionInput::new(acurast_validator_ink::VERIFY_PROOF_SELECTOR)
                            .push_arg(snapshot)
                            .push_arg(proof),
                    )
                    .transferred_value(0)
                    .returns()
                    .try_invoke();

            match call_result {
                // Errors from the underlying execution environment (e.g the Contracts pallet)
                Err(error) => Err(Error::Verbose(format!("{:?}", error))),
                // Errors from the programming language
                Ok(Err(error)) => Err(Error::LangError(error)),
                // Errors emitted by the contract being called
                Ok(Ok(Err(error))) => Err(Error::ValidatorError(error)),
                // Proof is not valid
                Ok(Ok(Ok(is_valid))) if !is_valid => Err(Error::InvalidProof),
                // Proof is valid
                Ok(Ok(Ok(_))) => {
                    // The proof is valid
                    for action in actions {
                        // Verify if message was already processed and fail if it was
                        assert!(
                            !self.processed_incoming_actions.contains(action.id),
                            "INVALID_INCOMING_ACTION_ID"
                        );
                        self.processed_incoming_actions.insert(action.id, &());

                        // Process action
                        match action.payload {
                            VersionedIncomingActionPayload::V1(
                                IncomingActionPayloadV1::AssignJobProcessor(payload),
                            ) => {
                                match JobInformation::decode(self, payload.job_id)? {
                                    JobInformation::V1(mut job) => {
                                        let processor_address = AccountId::from(payload.processor);
                                        // Update the processor list for the given job
                                        job.processors.push(processor_address);

                                        // Send initial fees to the processor (the processor may need a reveal)
                                        let initial_fee = job.expected_fulfillment_fee;
                                        job.remaining_fee = job.remaining_fee - initial_fee;
                                        // Transfer
                                        self.env()
                                            .transfer(processor_address, initial_fee)
                                            .expect("COULD_NOT_TRANSFER");

                                        if job.processors.len() == (job.slots as usize) {
                                            job.status = StatusKind::Assigned;
                                        }

                                        // Save changes
                                        self.job_info.insert(
                                            payload.job_id,
                                            &(Version::V1 as u16, job.encode()),
                                        );

                                        Ok(())
                                    }
                                }
                            }
                            VersionedIncomingActionPayload::V1(
                                IncomingActionPayloadV1::FinalizeJob(payload),
                            ) => {
                                match JobInformation::decode(self, payload.job_id)? {
                                    JobInformation::V1(mut job) => {
                                        // Update job status
                                        job.status = StatusKind::FinalizedOrCancelled;

                                        assert!(
                                            payload.unused_reward <= job.maximum_reward,
                                            "ABOVE_MAXIMUM_REWARD"
                                        );

                                        let refund = job.remaining_fee + payload.unused_reward;
                                        if refund > 0 {
                                            self.env()
                                                .transfer(job.creator, refund)
                                                .expect("COULD_NOT_TRANSFER");
                                        }

                                        // Save changes
                                        self.job_info.insert(
                                            payload.job_id,
                                            &(Version::V1 as u16, job.encode()),
                                        );

                                        Ok(())
                                    }
                                }
                            }
                            VersionedIncomingActionPayload::V1(IncomingActionPayloadV1::Noop) => {
                                // Intentionally do nothing
                                Ok(())
                            }
                        }?;

                        // Emit event informing that a given incoming message has been processed
                        EmitEvent::<Self>::emit_event(
                            self.env(),
                            IncomingActionProcessed {
                                action_id: action.id,
                            },
                        );
                    }

                    Ok(())
                }
            }
        }

        #[ink(message)]
        pub fn fulfill(&mut self, job_id: u128, payload: Vec<u8>) -> Result<(), Error> {
            self.fail_if_paused()?;

            match JobInformation::decode(self, job_id)? {
                JobInformation::V1(mut job) => {
                    // Verify if sender is assigned to the job
                    if !job.processors.contains(&self.env().caller()) {
                        return Err(Error::NotJobProcessor);
                    }

                    // Verify that the job has not been finalized
                    if job.status != StatusKind::Assigned {
                        return Err(Error::JobAlreadyFinished);
                    }

                    // Re-fill processor fees
                    // Forbidden to credit 0ꜩ to a contract without code.
                    let has_funds = job.remaining_fee >= job.expected_fulfillment_fee;
                    let next_execution_fee = if has_funds && job.expected_fulfillment_fee > 0 {
                        job.remaining_fee -= job.expected_fulfillment_fee;

                        job.expected_fulfillment_fee
                    } else {
                        0
                    };

                    // Pass the fulfillment to the destination contract
                    let call_result: OuterError<acurast_consumer_ink::FulfillReturn> =
                        build_call::<DefaultEnvironment>()
                            .call(job.destination)
                            .exec_input(
                                ExecutionInput::new(acurast_consumer_ink::FULFILL_SELECTOR)
                                    .push_arg(job_id)
                                    .push_arg(payload),
                            )
                            .transferred_value(next_execution_fee)
                            .returns()
                            .try_invoke();

                    match call_result {
                        // Errors from the underlying execution environment (e.g the Contracts pallet)
                        Err(error) => Err(Error::Verbose(format!("{:?}", error))),
                        // Errors from the programming language
                        Ok(Err(error)) => Err(Error::LangError(error)),
                        // Errors emitted by the contract being called
                        Ok(Ok(Err(error))) => Err(Error::ConsumerError(error)),
                        // Successful call result
                        Ok(Ok(Ok(()))) => {
                            // Save changes
                            self.job_info
                                .insert(job_id, &(Version::V1 as u16, job.encode()));

                            Ok(())
                        }
                    }
                }
            }
        }

        //
        // Views
        //

        #[ink(message)]
        pub fn action_info(&self, action_id: u64) -> Option<(u64, u128, Vec<u8>)> {
            self.actions.get(action_id)
        }

        #[ink(message)]
        pub fn is_action_processed(&self, action_id: u64) -> bool {
            self.processed_incoming_actions.contains(action_id)
        }

        /// The purpose of this method is to generate proofs for outgoing actions
        #[ink(message)]
        pub fn generate_proof(&self, from: u64, to: u64) -> Result<MerkleProof<[u8; 32]>, Error> {
            // Validate arguments
            if from == 0 || to == 0 {
                return Err(Error::Verbose("`from/to` cannot be zero".to_string()));
            }
            if to >= self.next_outgoing_action_id {
                return Err(Error::Verbose(
                    "`to` should be less then `next_action_id`".to_string(),
                ));
            }

            // Normalize leaf position: leafs start on position 0, but actions id's start from 1
            let from_id = from;
            let to_id = to;

            // Prepare a range of actions for generating the proof
            let positions: Vec<u64> = (from_id..=to_id).collect();
            let leaf_index: Vec<u64> = positions
                .iter()
                .map(|action_id| match self.actions.get(action_id) {
                    None => Err(Error::UnknownActionIndex(*action_id)),
                    Some((leaf_index, _, _)) => Ok(leaf_index),
                })
                .collect::<Result<Vec<u64>, Error>>()?;

            // Generate proof
            let call_result: OuterError<acurast_state_ink::GenerateProofReturn> =
                build_call::<DefaultEnvironment>()
                    .call(self.config.merkle_aggregator)
                    .exec_input(
                        ExecutionInput::new(acurast_state_ink::GENERATE_PROOF_SELECTOR)
                            .push_arg(leaf_index.clone()),
                    )
                    .transferred_value(0)
                    .returns()
                    .try_invoke();

            match call_result {
                // Errors from the underlying execution environment (e.g the Contracts pallet)
                Err(error) => Err(Error::Verbose(format!("{:?}", error))),
                // Errors from the programming language
                Ok(Err(error)) => Err(Error::LangError(error)),
                // Errors emitted by the contract being called
                Ok(Ok(Err(error))) => Err(Error::StateAggregatorError(error)),
                // Successful call result
                Ok(Ok(Ok(proof))) => {
                    let leaves: Vec<LeafProof> = positions
                        .iter()
                        .map(|action_id| match self.actions.get(action_id) {
                            None => Err(Error::UnknownActionIndex(*action_id)),
                            Some((leaf_index, _snapshot, data)) => {
                                Ok(LeafProof { leaf_index, data })
                            }
                        })
                        .collect::<Result<Vec<LeafProof>, Error>>()?;

                    // Prepare result
                    Ok(MerkleProof {
                        mmr_size: proof.mmr_size,
                        proof: proof.proof,
                        leaves,
                    })
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use hex_literal::hex;

        /// Imports all the definitions from the outer scope so we can use them here.
        use super::*;

        #[ink::test]
        fn test_action_encoding() {
            let encoded_incoming_action = hex!("00000000000000000002");

            let decoded_incoming_action =
                IncomingAction::decode(&mut encoded_incoming_action.as_slice());

            assert_eq!(
                decoded_incoming_action.unwrap(),
                IncomingAction {
                    id: 0,
                    payload: VersionedIncomingActionPayload::V1(IncomingActionPayloadV1::Noop)
                }
            );
        }
    }
}
