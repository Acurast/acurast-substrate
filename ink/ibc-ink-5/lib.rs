#![cfg_attr(not(feature = "std"), no_std, no_main)]
use ink::{env::call::Selector};

// Method selectors

pub const FULFILL_SELECTOR: Selector = Selector::new(ink::selector_bytes!("send"));

#[ink::contract]
pub mod ibc {
    use ink::prelude::string::String;
    use ink::prelude::vec::Vec;
    use scale::{Decode, Encode};
    use ink::storage::{Mapping, Lazy};
    use ink::{prelude::collections::BTreeSet};
    use scale_info::prelude::vec;
    use ink::env::{hash::{Blake2x256}, call::{build_call, Selector, ExecutionInput}, DefaultEnvironment};

    pub type PubKey = [u8; 32];
    pub type MessageIndex = u64;
    pub type MessageId = [u8; 32];
    pub type MessageNonce = [u8; 32];
    pub type FunctionName = String;
    pub type Payload = Vec<u8>;
    pub type Contract = AccountId;
    pub type Signature = [u8; 65];
    pub type Public = Vec<u8>;
    pub type Signatures = Vec<Signature>;

    #[ink(storage)]
    pub struct Ibc {
        config: Config,
        /// outgoing messages
        outgoing: Mapping<MessageId, OutgoingMessageWithMeta>,
        /// iterable index of outgoing messages needed for discovery by relayer
        outgoing_index: Lazy<Vec<MessageId>>,
        /// incoming messages 
        incoming: Mapping<MessageId, IncomingMessageWithMeta>,
        /// iterable index of incoming messages needed for discovery by relayer
        incoming_index:  Lazy<Vec<MessageId>>,
        message_counter: u128,
        // because [`ink::storage::traits::StorageLayout`] is not implemented for [u8; 33], we use Vec<u8>
        // see https://substrate.stackexchange.com/questions/5786/ink-smart-contract-struct-field-issues
        oracle_public_keys: Mapping<Public, ()>,
    }

    /// Contract configurations are contained in this structure
    #[derive(Clone, Eq, PartialEq, Encode, Decode)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub struct Config {
        /// Address allowed to manage the contract
        owner: AccountId,
        /// Flag that states if the contract is paused or not
        paused: bool,
        min_delivery_signatures: u8,
        min_receipt_signatures: u8,
        min_ttl: BlockNumber,
        /// ttl for incoming message before removed from ids index (to limit length of vector when reading `incoming_index`)
        incoming_ttl: BlockNumber,
    }

    impl Ibc {
        #[ink(constructor)]
        pub fn default() -> Self {
            let mut d = Self {
                config: Config {
                    owner: AccountId::from([24,90,139,95,146,236,211,72,237,155,18,160,71,202,43,40,72,139,19,152,6,90,141,255,141,207,136,98,69,249,40,11]),
                    paused: false,
                    min_delivery_signatures: 1,
                    min_receipt_signatures: 1,
                    min_ttl: 20,
                    incoming_ttl: 30,
                },
                outgoing: Default::default(),
                outgoing_index: Default::default(),
                incoming: Default::default(),
                incoming_index: Default::default(),
                message_counter: 0,
                oracle_public_keys: Default::default(),
            };
            d.oracle_public_keys.insert(vec![3,165,118,76,57,181,62,211,167,24,6,116,158,212,202,14,15,197,104,143,109,3,235,177,22,180,132,216,84,109,107,213,199], &());
            d
        }

        #[ink(message)]
        pub fn configure(&mut self, actions: Vec<ConfigureArgument>) -> Result<(), Error> {
            self.ensure_owner()?;

            for action in actions {
                match action {
                    ConfigureArgument::Owner(address) => self.config.owner = address,
                    ConfigureArgument::Paused(paused) => self.config.paused = paused,
                    ConfigureArgument::Code(code_hash) => self.set_code(code_hash),
                    ConfigureArgument::OraclePublicKeys(oracle_updates) => self.update_oracles(oracle_updates),
                    ConfigureArgument::MinDeliverySignatures(min_delivery_signatures) => self.config.min_delivery_signatures = min_delivery_signatures,
                    ConfigureArgument::MinReceiptSignatures(min_receipt_signatures) => self.config.min_receipt_signatures = min_receipt_signatures,
                    ConfigureArgument::MinTTL(min_ttl) => self.config.min_ttl = min_ttl,
                    ConfigureArgument::IncomingTTL(incoming_ttl) => self.config.incoming_ttl = incoming_ttl,
                }
            }

            Ok(())
        }

        #[ink(message)]
        pub fn config(&self) -> Config {
            self.config.clone()
        }

        #[ink(message)]
        pub fn message_count(&self) -> u128 {
            self.message_counter
        }

        #[ink(message)]
        pub fn outgoing_message(&self, message_id: MessageId) -> Option<OutgoingMessageWithMeta> {
            self.outgoing.get(message_id)
        }
       
        #[ink(message)]
        pub fn outgoing_index(&self) -> Vec<MessageId> {
            self.outgoing_index.get_or_default()
        }
        
        #[ink(message)]
        pub fn incoming_message(&self, message_id: MessageId) -> Option<IncomingMessageWithMeta> {
            self.incoming.get(message_id)
        }
       
        #[ink(message)]
        pub fn incoming_index(&self) -> Vec<MessageId> {
            self.incoming_index.get_or_default()
        }

        #[ink(message)]
        pub fn oracles(&self, public: Public) -> bool {
            self.oracle_public_keys.get(public).is_some()
        }

        fn update_oracles(&mut self, updates: Vec<OracleUpdate>) {
            // Process actions
            let (added, removed) =
                updates.into_iter().fold((vec![], vec![]), |acc, action| {
                    let (added, removed) = acc;
                    match action {
                        OracleUpdate::Add(public) => {
                            self.oracle_public_keys.insert(&public, &());
                        },
                        OracleUpdate::Remove(public) => {
                            self.oracle_public_keys.remove(&public);
                        },
                    }
                    (added, removed)
                });

            Self::env().emit_event(OraclesUpdated {
                added,
                removed,
            });
        }

        /// Modifies the code which is used to execute calls to this contract.
        fn set_code(&mut self, code_hash: Hash) {
            ink::env::set_code_hash::<DefaultEnvironment>(&code_hash).unwrap_or_else(|err| {
                panic!("Failed to `set_code_hash` to {:?} due to {:?}", code_hash, err)
            });
            ink::env::debug_println!("Switched code hash to {:?}.", code_hash);
        }

        /// Sends a (test) message with the origin of the extrinsic call as the payload.
        #[ink(message, payable)]
        pub fn send_message(
            &mut self,
            recipient: Subject,
            ttl: BlockNumber,
        ) -> Result<(), Error> {
            self.ensure_unpaused()?;

            // https://use.ink/macros-attributes/payable/
            let fee = self.env().transferred_value();

            let count = self.message_counter;
            self.message_counter = count + 1;

            let message = self.do_send_message(
                Subject::AlephZero(Layer::Contract(ContractCall {contract: self.env().account_id(), selector: Some(ink::selector_bytes!("send_message"))})),
                self.env().caller(),
                self.env().hash_encoded::<Blake2x256, _>(&count),
                recipient,
                // the test message payload is just the sender of the message
                self.env().caller().encode(),
                ttl,
                fee,
            )?;

            Self::env().emit_event(MessageReadyToSend {
                message
            });

            Ok(())
        }

        /// Used by a relayer to confirm that a message has been delivered, claiming the message fee.
        #[ink(message)]
        pub fn confirm_message_delivery(
            &mut self,
            // We only pass the id and retrieve message from runtime storage to ensure the signatures are over the message originally sent (+ the relayer's address).
            id: MessageId,
            // Signatures confirming the message delivery within `ttl_block`.
            //
            // Can be left empty if the message was not delivered in time. In this case the fee is paid back to sender (not necessarily the origin of this call).
            signatures: Signatures,
        ) -> Result<(), Error> {
            self.ensure_unpaused()?;

            // https://use.ink/basics/contract-debugging/
            ink::env::debug_println!("Confirm message delivery of {}", hex::encode(id));

            let message = self.outgoing.get(&id).ok_or(Error::MessageNotFound)?;

            let current_block = self.env().block_number();

            if message.ttl_block < current_block {
                Err(Error::DeliveryConfirmationOverdue)?
            };

            let relayer = self.env().caller();
            self.check_signatures(&message.message, Some(relayer), signatures, self.config.min_delivery_signatures)?;

            // https://github.com/use-ink/ink-examples/blob/main/contract-transfer/lib.rs#L29
            if self.env().transfer(message.payer, message.fee).is_err() {
                panic!(
                    "SEVERE: Paying fee to relayer failed. This can be the case if the contract does not\
                     have sufficient free funds or if the transfer would have brought the\
                     contract's balance below minimum balance."
                )
            }

            self.outgoing.remove(&id);

            let mut index = self.outgoing_index.get_or_default();
            index.retain(|&i| i != id);
            self.outgoing_index.set(&index);

            Self::env().emit_event(MessageDelivered { id });

            Ok(())
        }

        /// Sends a message by the given `sender` paid by a potentially different `payer`.
        ///
        /// **NOTE**: This is an internal function but could be made available
        /// to other contracts if the authorization for the passed `sender` has been ensured at the caller.
        /// Be careful to not allow for unintended impersonation.
        ///
        /// **NOTE**: the fee paid by `payer` is supposed to be transferred to this contract by the caller, e.g.
        /// by means of a `payable` message.
        fn do_send_message(
            &mut self,
            sender: Subject,
            payer: AccountId,
            nonce: MessageNonce,
            recipient: Subject,
            payload: Payload,
            ttl: BlockNumber,
            fee: Balance,
        ) -> Result<OutgoingMessageWithMeta, Error> {
            self.ensure_unpaused()?;

            let current_block = self.env().block_number();

            let id = self.message_id(&sender, nonce);

            // https://use.ink/basics/contract-debugging/
            ink::env::debug_println!("Send message {}", hex::encode(id));

            // look for duplicates
            if let Some(message) = self.outgoing.get(&id) {
                // potential duplicate found: check for ttl
                if message.ttl_block >= current_block {
                    Err(Error::MessageWithSameNoncePending)?;
                }

                // continue below and overwrite message
            }

            // validate params
            if ttl < self.config.min_ttl {
                Err(Error::TTLSmallerThanMinimum)?
            }

            let message = Message { id, sender, nonce, recipient, payload };
            let message_with_meta = OutgoingMessageWithMeta {
                message,
                current_block,
                ttl_block: current_block + ttl,
                fee,
                payer: payer.clone(),
            };
            
            self.outgoing.insert(&id, &message_with_meta);

            let mut index = self.outgoing_index.get_or_default();
            index.push(id);
            self.outgoing_index.set(&index);

            Ok(message_with_meta)
        }

        /// Receives messages signed by the oracles.
        #[ink(message)]
        pub fn receive_message(
            &mut self,
            sender: Subject,
            nonce: MessageNonce,
            recipient: Subject,
            payload: Payload,
            signatures: Signatures,
        ) -> Result<(), Error> {
            self.ensure_unpaused()?;

            let contract_call = if let Subject::AlephZero(layer) = recipient.clone() {
                if let Layer::Contract(contract_call) = layer {
                    Ok(contract_call)
                } else {
                    Err(Error::IncorrectRecipient)
                }
            } else {
                Err(Error::IncorrectRecipient)
            }?;

            let id = self.message_id(&sender, nonce);
            // https://use.ink/basics/contract-debugging/
            ink::env::debug_println!("Receive message {}", hex::encode(id));
            if self.incoming.get(&id).is_some() {
                Err(Error::MessageAlreadyReceived)?
            }

            let current_block = self.env().block_number();
            let relayer = self.env().caller();

            let message =
                Message { id, sender, nonce, recipient, payload };

            self.check_signatures(&message, None, signatures, self.config.min_receipt_signatures)?;

            let message_with_meta =
                IncomingMessageWithMeta { message, current_block, relayer };

            self.incoming.insert(&id, &message_with_meta);

            if let Some(selector) = contract_call.selector {
                match build_call::<DefaultEnvironment>()
                    .call(contract_call.contract)
                    .ref_time_limit(0)
                    .proof_size_limit(0)
                    .storage_deposit_limit(0)
                    .transferred_value(0)
                    .exec_input(
                        ExecutionInput::new(Selector::new(selector))
                            .push_arg(message_with_meta.message.payload)
                    )
                    .returns::<()>()
                    .try_invoke() {
                    Ok(_) => {
                        Self::env().emit_event(MessageProcessed { id });
                    }
                    Err(_) => {
                        // swallow error to make storing message persistent
                        Self::env().emit_event(MessageProcessedWithErrors { id });
                    }
                }
            } else {
                Self::env().emit_event(MessageProcessed { id });
            }

            Ok(())
        }

        /// Used by a relayer to clean incoming index from messages older than `Config::incoming_ttl`. Currently it does not clean the actual message store `incoming`, so duplicates are still detected.
        #[ink(message)]
        pub fn clean_incoming_index(
            &mut self,
        ) -> Result<(), Error> {
            self.ensure_unpaused()?;

            let current_block = self.env().block_number();

            let mut index = self.incoming_index.get_or_default();
            index.retain(|&i| {
                let retain = if let Some(message) = self.incoming.get(&i) {
                    current_block < self.config.incoming_ttl || message.current_block > current_block - self.config.incoming_ttl
                } else {
                    // if message store does not have this message, delete it from index
                    false
                };
                
                if !retain {
                    // https://use.ink/basics/contract-debugging/
                    ink::env::debug_println!("Clear message from index: {}", hex::encode(i));
                }

                retain
            });
            self.incoming_index.set(&index);

            Ok(())
        }

        fn message_id(&mut self, sender: &Subject, nonce: MessageNonce) -> MessageId {
            // https://docs.rs/ink_env/4.2.0/ink_env/fn.hash_encoded.html
            self.env().hash_encoded::<Blake2x256, _>(&(sender, nonce))
        }

        fn check_signatures(&mut self, message: &Message, relayer: Option<AccountId>, signatures: Signatures, min_signatures: u8) -> Result<(), Error> {
            if signatures.len() < min_signatures.into() {
                Err(Error::NotEnoughSignaturesValid)?
            }

            let mut seen: BTreeSet<Vec<u8>> = Default::default();
            signatures.into_iter().try_for_each(
                |signature| -> Result<(), Error> {
                    ink::env::debug_println!("checking signature: {}", hex::encode(&signature));

                    let message_hash: [u8; 32] = if let Some(r) = &relayer {
                        // https://docs.rs/ink_env/4.2.0/ink_env/fn.hash_bytes.html
                        self.env().hash_bytes::<Blake2x256>(&(message, r).encode())
                    } else {
                        // https://docs.rs/ink_env/4.2.0/ink_env/fn.hash_bytes.html
                        self.env().hash_bytes::<Blake2x256>(&message.encode())
                    };
                    ink::env::debug_println!("message_hash: {}", hex::encode(message_hash));
                    let public: Public = self.env().ecdsa_recover(&signature, &message_hash).map_err(|_|
                        Error::SignatureInvalid
                    )?.to_vec();

                    ink::env::debug_println!("recovered public key: {}", hex::encode(&public));

                    self.oracle_public_keys.get(&public).ok_or(
                        Error::PublicKeyUnknown
                    )?;

                    if seen.contains(&public) {
                        Err(Error::DuplicateSignature)?
                    }
                    seen.insert(public);

                    Ok(())
                },
            )?;

            Ok(())
        }

        fn ensure_owner(&self) -> Result<(), Error> {
            if self.config.owner.eq(&self.env().caller()) {
                Ok(())
            } else {
                Err(Error::NotOwner)
            }
        }

        fn ensure_unpaused(&self) -> Result<(), Error> {
            if self.config.paused {
                Err(Error::ContractPaused)
            } else {
                Ok(())
            }
        }
    }

    #[derive(Clone, Eq, PartialEq, Encode, Decode)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub enum ConfigureArgument {
        Owner(AccountId),
        Paused(bool),
        Code(Hash),
        OraclePublicKeys(Vec<OracleUpdate>),
        MinDeliverySignatures(u8),
        MinReceiptSignatures(u8),
        MinTTL(BlockNumber),
        IncomingTTL(BlockNumber),
    }

    #[derive(Clone, Eq, PartialEq, Encode, Decode)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub enum Error {
        ContractPaused,
        NotOwner,
        IncorrectRecipient,
        MessageAlreadyReceived,
        PublicKeyUnknown,
        DuplicateSignature,
        SignatureInvalid,
        NotEnoughSignaturesValid,
        MessageWithSameNoncePending,
        TTLSmallerThanMinimum,
        MessageNotFound,
        DeliveryConfirmationOverdue
    }

    #[ink(event)]
    pub struct OraclesUpdated {
        added: Vec<Public>,
        removed: Vec<Public>,
    }

    #[ink(event)]
    pub struct MessageReadyToSend {
        message: OutgoingMessageWithMeta,
    }

    #[ink(event)]
    pub struct MessageDelivered {
        id: MessageId,
    }

    #[ink(event)]
    pub struct MessageRemoved {
        id: MessageId,
    }

    #[ink(event)]
    pub struct MessageStored {
        id: MessageId,
    }

    #[ink(event)]
    pub struct MessageProcessed {
        id: MessageId,
    }

    #[ink(event)]
    pub struct MessageProcessedWithErrors {
        id: MessageId,
    }

    #[derive(Clone, Eq, PartialEq, Encode, Decode)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub enum OracleUpdate {
        Add(Public),
        Remove(Public),
    }

    #[derive(Clone, Eq, PartialEq, Encode, Decode)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub struct OutgoingMessageWithMeta{
        pub message: Message,
        pub current_block: BlockNumber,
        pub ttl_block: BlockNumber,
        pub fee: Balance,
        /// The payer of the fee. Not necessarily the sender of the message.
        pub payer: AccountId,
    }

    #[derive(Clone, Eq, PartialEq, Encode, Decode)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub struct IncomingMessageWithMeta {
        pub message: Message,
        pub current_block: BlockNumber,
        pub relayer: AccountId,
    }

    #[derive(Clone, Eq, PartialEq, Encode, Decode)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub struct Message {
        pub id: MessageId,
        pub sender: Subject,
        pub nonce: MessageNonce,
        pub recipient: Subject,
        pub payload: Payload,
        // pub amount: u128,
    }

    #[derive(Clone, Eq, PartialEq, Encode, Decode)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub struct MessageBody {
        pub sender: Subject,
        pub recipient: Subject,
        pub payload: Payload,
        // pub amount: u128,
    }

    impl From<Message> for MessageBody {
        fn from(m: Message) -> Self {
            MessageBody { sender: m.sender, recipient: m.recipient, payload: m.payload }
        }
    }

    #[derive(Clone, Eq, PartialEq, Encode, Decode)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub enum Subject {
        Acurast(Layer),
        AlephZero(Layer),
    }

    #[derive(Clone, Eq, PartialEq, Encode, Decode)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub enum Layer {
        /// A sender/recipient extrinsic. In case of a sender, it should hold the pallet_account of either this pallet
        /// if `hyperdrive_ibc::send_message`-extrinsic sent the message or the (internal) caller of `hyperdrive_ibc::do_send_message`.
        Extrinsic(AccountId),
        Contract(ContractCall),
    }

    /// https://use.ink/4.x/basics/cross-contract-calling#callbuilder
    #[derive(Clone, Eq, PartialEq, Encode, Decode)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub struct ContractCall {
        pub contract: Contract,
        /// Selector for the message of `contract` to send payload to,
        /// as the only argument.
        pub selector: Option<[u8; 4]>,
    }
}
