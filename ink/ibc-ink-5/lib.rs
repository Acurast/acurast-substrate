#![cfg_attr(not(feature = "std"), no_std, no_main)]
use ink::{env::call::Selector};

// Method selectors

pub const FULFILL_SELECTOR: Selector = Selector::new(ink::selector_bytes!("send"));

#[ink::contract]
pub mod ibc {
    use ink::prelude::string::String;
    use ink::prelude::vec::Vec;
    use scale::{Decode, Encode};
    use ink::storage::Mapping;
    use ink::storage::Lazy;
    use ink_env::hash::Blake2x256;
    use ink::{prelude::collections::BTreeSet};
    use scale_info::prelude::vec;

    pub type PubKey = [u8; 32];
    pub type MessageIndex = u64;
    pub type MessageId = [u8; 32];
    pub type MessageNonce = [u8; 32];
    pub type FunctionName = String;
    pub type Payload = Vec<u8>;
    pub type Contract = AccountId;
    pub type Signature = [u8; 64];
    pub type Public = [u8; 32];
    pub type Signatures = Vec<(Signature, Public)>;

    #[ink(storage)]
    pub struct Ibc {
        config: Config,
        outgoing_messages: Mapping<MessageId, OutgoingMessageWithMeta>,
        incoming_messages: Mapping<MessageId, IncomingMessageWithMeta>,
        message_counter: u128,
        oracle_public_keys: Lazy<BTreeSet<Public>>,
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
    }

    impl Ibc {
        #[ink(constructor)]
        pub fn default() -> Self {
            Self {
                config: Config {
                    owner: AccountId::from([0x0; 32]),
                    paused: false,
                    min_delivery_signatures: 1,
                    min_receipt_signatures: 1,
                    min_ttl: 20,
                },
                outgoing_messages: Default::default(),
                incoming_messages: Default::default(),
                message_counter: 0,
                oracle_public_keys: Default::default(),
            }
        }

        #[ink(message)]
        pub fn configure(&mut self, actions: Vec<ConfigureArgument>) -> Result<(), Error> {
            self.ensure_owner()?;

            for action in actions {
                match action {
                    ConfigureArgument::SetOwner(address) => self.config.owner = address,
                    ConfigureArgument::SetPaused(paused) => self.config.paused = paused,
                    ConfigureArgument::SetCode(code_hash) => self.set_code(code_hash),
                    ConfigureArgument::MinDeliverySignatures(min_delivery_signatures) => self.config.min_delivery_signatures = min_delivery_signatures,
                    ConfigureArgument::MinReceiptSignatures(min_receipt_signatures) => self.config.min_receipt_signatures = min_receipt_signatures,
                    ConfigureArgument::MinTTL(min_ttl) => self.config.min_ttl = min_ttl,
                }
            }

            Ok(())
        }

        #[ink(message)]
        pub fn update_oracles(&mut self, updates: Vec<OracleUpdate>) -> Result<(), Error> {
            self.ensure_owner()?;

            // Process actions
            let (added, removed) =
                updates.into_iter().fold((vec![], vec![]), |acc, action| {
                    let (mut added, mut removed) = acc;
                    match action {
                        OracleUpdate::Add(public) => {
                            if !self.oracle_public_keys.get_or_default().contains(&public) {
                                self.oracle_public_keys.get_or_default().insert(public.clone());
                                added.push(public)
                            }
                        },
                        OracleUpdate::Remove(public) => {
                            if self.oracle_public_keys.get_or_default().contains(&public) {
                                self.oracle_public_keys.get_or_default().remove(&public);
                                removed.push(public)
                            }
                        },
                    }
                    (added, removed)
                });

            Self::env().emit_event(OraclesUpdated {
                added,
                removed,
            });

            Ok(())
        }

        /// Modifies the code which is used to execute calls to this contract.
        fn set_code(&mut self, code_hash: Hash) {
            ink::env::set_code_hash::<Environment>(&code_hash).unwrap_or_else(|err| {
                panic!("Failed to `set_code_hash` to {:?} due to {:?}", code_hash, err)
            });
            ink::env::debug_println!("Switched code hash to {:?}.", code_hash);
        }

        /// Sends a message with sender being the calling address of this message.
        #[ink(message, payable)]
        pub fn send_message(
            &mut self,
            recipient: Subject,
            payload: Payload,
            ttl: BlockNumber,
        ) -> Result<(), Error> {
            self.ensure_unpaused()?;

            // https://use.ink/macros-attributes/payable/
            let fee = self.env().transferred_value();

            let count = self.message_counter;
            self.message_counter = count + 1;

            let message = self.do_send_message(
                Subject::Acurast(Layer::Contract(self.env().caller())),
                self.env().caller(),
                self.env().hash_encoded::<Blake2x256, _>(&count),
                recipient,
                payload,
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

            let message = self.outgoing_messages.get(&id).ok_or(Error::MessageNotFound)?;

            let current_block = self.env().block_number();

            if message.ttl_block < current_block {
                Err(Error::DeliveryConfirmationOverdue)?
            };

            self.check_signatures(&message.message, signatures, self.config.min_delivery_signatures)?;

            // https://github.com/use-ink/ink-examples/blob/main/contract-transfer/lib.rs#L29
            if self.env().transfer(message.payer, message.fee).is_err() {
                panic!(
                    "SEVERE: Paying fee to relayer failed. This can be the case if the contract does not\
                     have sufficient free funds or if the transfer would have brought the\
                     contract's balance below minimum balance."
                )
            }

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
            if let Some(message) = self.outgoing_messages.get(&id) {
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
            self.outgoing_messages.insert(&id, &message_with_meta);

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

            if let Subject::Acurast(multi) = &recipient {
                if let Layer::Contract(_) = multi {
                    Ok(())
                } else {
                    Err(Error::IncorrectRecipient)
                }
            } else {
                Err(Error::IncorrectRecipient)
            }?;

            let id = self.message_id(&recipient, nonce);
            // https://use.ink/basics/contract-debugging/
            ink::env::debug_println!("Receive message {}", hex::encode(id));
            if self.incoming_messages.get(&id).is_some() {
                Err(Error::MessageAlreadyReceived)?
            }

            let current_block = self.env().block_number();

            let message =
                Message { id, sender, nonce, recipient, payload };

            self.check_signatures(&message, signatures, self.config.min_receipt_signatures)?;

            let message_with_meta =
                IncomingMessageWithMeta { message, current_block };

            self.incoming_messages.insert(&id, &message_with_meta);

            Self::env().emit_event(MessageProcessed { id });

            Ok(())
        }

        fn message_id(&mut self, subject: &Subject, nonce: MessageNonce) -> MessageId {
            // https://docs.rs/ink_env/5.0.0/ink_env/fn.hash_encoded.html
            self.env().hash_encoded::<Blake2x256, _>(&(subject, nonce))
        }

        fn check_signatures(&mut self, message: &Message, signatures: Signatures, min_signatures: u8) -> Result<(), Error> {
            if signatures.len() < min_signatures.into() {
                Err(Error::NotEnoughSignaturesValid)?
            }

            signatures.into_iter().try_for_each(
                |(signature, public)| -> Result<(), Error> {
                    self.oracle_public_keys.get_or_default().get(&public).ok_or(
                        Error::PublicKeyUnknown
                    )?;

                    // https://docs.rs/ink_env/5.0.0/ink_env/fn.sr25519_verify.html
                    self.env().sr25519_verify(&signature, &message.encode(), &public).map_err(|_|
                        Error::SignatureInvalid
                    )?;

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
        SetOwner(AccountId),
        SetPaused(bool),
        SetCode(Hash),
        MinDeliverySignatures(u8),
        MinReceiptSignatures(u8),
        MinTTL(BlockNumber),
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
    pub struct MessageProcessed {
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
        Extrinsic(RawOrigin),
        Contract(Contract),
    }

    #[derive(Clone, Eq, PartialEq, Encode, Decode)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub enum RawOrigin {
        /// The system itself ordained this dispatch to happen: this is the highest privilege level.
        Root,
        /// It is signed by some public key and we provide the `AccountId`.
        Signed(AccountId),
        /// It is signed by nobody, can be either:
        /// * included and agreed upon by the validators anyway,
        /// * or unsigned transaction validated by a pallet.
        None,
    }
}
