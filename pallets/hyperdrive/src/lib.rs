#![cfg_attr(not(feature = "std"), no_std)]

extern crate core;

pub use pallet::*;
pub use traits::*;
pub use types::*;

#[cfg(test)]
mod ethereum_tests;
#[cfg(test)]
mod mock;
#[cfg(any(test, feature = "runtime-benchmarks"))]
mod stub;
#[cfg(test)]
mod substrate_tests;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
mod traits;

pub mod chain;
pub mod instances;

mod types;
pub mod weights;

#[frame_support::pallet]
pub mod pallet {
    use core::{fmt::Debug, str::FromStr};

    use frame_support::dispatch::PostDispatchInfo;
    use frame_support::traits::Get;
    use frame_support::{
        pallet_prelude::*,
        sp_runtime::traits::{
            AtLeast32BitUnsigned, Bounded, CheckEqual, MaybeDisplay, SimpleBitOps,
        },
    };
    use frame_support::{transactional, BoundedBTreeSet};
    use frame_system::pallet_prelude::*;
    use pallet_acurast::ParameterBound;
    use sp_arithmetic::traits::{CheckedRem, Zero};
    use sp_core::H256;
    use sp_runtime::traits::Hash;
    use sp_std::prelude::*;
    use sp_std::vec;

    use super::*;

    /// A instantiable pallet for receiving secure state synchronizations into Acurast.
    #[pallet::pallet]
    pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

    /// Configures the pallet instance for a specific target chain from which we synchronize state into Acurast.
    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config + pallet_acurast::Config {
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type ParsableAccountId: Into<<Self as frame_system::Config>::AccountId> + TryFrom<Vec<u8>>;
        type TargetChainOwner: Get<StateOwner>;
        /// The output of the `Hashing` function used to derive hashes of target chain state.
        type TargetChainHash: Parameter
            + Member
            + MaybeSerializeDeserialize
            + Debug
            + MaybeDisplay
            + SimpleBitOps
            + Ord
            + Default
            + Copy
            + CheckEqual
            + sp_std::hash::Hash
            + AsRef<[u8]>
            + AsMut<[u8]>
            + From<[u8; 32]>
            + MaxEncodedLen;
        /// The block number type used by the target runtime.
        type TargetChainBlockNumber: Parameter
            + Member
            + MaybeSerializeDeserialize
            + Debug
            + MaybeDisplay
            + AtLeast32BitUnsigned
            + Default
            + Bounded
            + Copy
            + sp_std::hash::Hash
            + FromStr
            + MaxEncodedLen
            + TypeInfo
            + Zero
            + From<u8>
            + CheckedRem;
        type Balance: Member
            + Parameter
            + AtLeast32BitUnsigned
            // required to translate Tezos Ints of unknown precision (Alternative: use Tezos SDK types in clients of this pallet)
            + From<u128>
            + Default
            + Copy
            + MaybeSerializeDeserialize
            + MaxEncodedLen
            + TypeInfo;
        type Proof: Parameter + Member + TypeInfo + Proof<Self, I>;

        /// The maximum transmitters accepted to submit a state root per snapshot.
        #[pallet::constant]
        type MaxTransmittersPerSnapshot: Get<u32> + ParameterBound;

        /// The hashing system (algorithm) being used in the runtime (e.g. Blake2).
        type TargetChainHashing: Hash<Output = H256> + TypeInfo;
        /// Transmission rate in blocks; `block % transmission_rate == 0` must hold.
        type TransmissionRate: Get<Self::TargetChainBlockNumber>;
        /// The quorum size of transmitters that need to agree on a state merkle root before accepting in proofs.
        ///
        /// **NOTE**: the quorum size must be larger than `ceil(number of transmitters / 2)`, otherwise multiple root hashes could become valid in terms of [`Pallet::validate_state_merkle_root`].
        type TransmissionQuorum: Get<u8>;

        type ActionExecutor: ActionExecutor<Self>;

        type WeightInfo: WeightInfo;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        StateTransmittersUpdate {
            added: Vec<(T::AccountId, types::ActivityWindow<BlockNumberFor<T>>)>,
            updated: Vec<(T::AccountId, types::ActivityWindow<BlockNumberFor<T>>)>,
            removed: Vec<T::AccountId>,
        },
        StateMerkleRootSubmitted {
            source: T::AccountId,
            snapshot: T::TargetChainBlockNumber,
            state_merkle_root: T::TargetChainHash,
        },
        StateMerkleRootAccepted {
            snapshot: T::TargetChainBlockNumber,
            state_merkle_root: T::TargetChainHash,
        },
        TargetChainOwnerUpdated {
            owner: StateOwner,
        },
        MessageProcessed(ProcessMessageResult),
    }

    /// This storage field maps the state transmitters to their respective activity window.
    ///
    /// These transmitters are responsible for submitting the merkle roots of supported
    /// source chains to acurast.
    #[pallet::storage]
    #[pallet::getter(fn state_transmitter)]
    pub type StateTransmitter<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128, T::AccountId, ActivityWindow<BlockNumberFor<T>>, ValueQuery>;

    #[pallet::type_value]
    pub fn FirstSnapshot<T: Config<I>, I: 'static>() -> T::TargetChainBlockNumber {
        1u8.into()
    }

    /// This storage field contains the latest validated snapshot number.
    #[pallet::storage]
    #[pallet::getter(fn latest_snapshot)]
    pub type CurrentSnapshot<T: Config<I>, I: 'static = ()> =
        StorageValue<_, T::TargetChainBlockNumber, ValueQuery, FirstSnapshot<T, I>>;

    /// This storage field contains the latest message identifier to have been transmitted.
    #[pallet::storage]
    #[pallet::getter(fn message_seq_id)]
    pub type MessageSequenceId<T: Config<I>, I: 'static = ()> =
        StorageValue<_, MessageIdentifier, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn state_merkle_root)]
    pub type StateMerkleRootCount<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
        _,
        Blake2_128,
        T::TargetChainBlockNumber,
        Identity,
        T::TargetChainHash,
        BoundedBTreeSet<T::AccountId, T::MaxTransmittersPerSnapshot>,
    >;

    #[pallet::type_value]
    pub fn FirstTargetChainOwner<T: Config<I>, I: 'static>() -> StateOwner {
        T::TargetChainOwner::get()
    }

    #[pallet::storage]
    #[pallet::getter(fn current_target_chain_owner)]
    pub type CurrentTargetChainOwner<T: Config<I>, I: 'static = ()> =
        StorageValue<_, StateOwner, ValueQuery, FirstTargetChainOwner<T, I>>;

    #[pallet::type_value]
    pub fn InitialTransmissionRate<T: Config<I>, I: 'static>() -> T::TargetChainBlockNumber {
        T::TransmissionRate::get()
    }

    #[pallet::storage]
    #[pallet::getter(fn current_transmission_rate)]
    pub type CurrentTransmissionRate<T: Config<I>, I: 'static = ()> =
        StorageValue<_, T::TargetChainBlockNumber, ValueQuery, InitialTransmissionRate<T, I>>;

    #[pallet::error]
    pub enum Error<T, I = ()> {
        /// A known transmitter submits outside the window of activity he is permissioned to.
        SubmitOutsideTransmitterActivityWindow,
        CalculationOverflow,
        UnexpectedSnapshot,
        ProofInvalid,
        ProofDoesNotMatch,
        MessageIdDoesNotMatch,
        InvalidMessageId,
    }

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        /// Used to add, update or remove state transmitters.
        #[pallet::call_index(0)]
        #[pallet::weight(< T as Config<I>>::WeightInfo::update_state_transmitters(actions.len() as u32))]
        pub fn update_state_transmitters(
            origin: OriginFor<T>,
            actions: StateTransmitterUpdates<T>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            // Process actions
            let (added, updated, removed) =
                actions
                    .iter()
                    .fold((vec![], vec![], vec![]), |acc, action| {
                        let (mut added, mut updated, mut removed) = acc;
                        match action {
                            StateTransmitterUpdate::Add(account, activity_window) => {
                                <StateTransmitter<T, I>>::set(
                                    account.clone(),
                                    activity_window.clone(),
                                );
                                added.push((account.clone(), activity_window.clone()))
                            }
                            StateTransmitterUpdate::Update(account, activity_window) => {
                                <StateTransmitter<T, I>>::set(
                                    account.clone(),
                                    activity_window.clone(),
                                );
                                updated.push((account.clone(), activity_window.clone()))
                            }
                            StateTransmitterUpdate::Remove(account) => {
                                <StateTransmitter<T, I>>::remove(account);
                                removed.push(account.clone())
                            }
                        }
                        (added, updated, removed)
                    });

            // Emit event to inform that the state transmitters were updated
            Self::deposit_event(Event::StateTransmittersUpdate {
                added,
                updated,
                removed,
            });

            Ok(PostDispatchInfo {
                actual_weight: None,
                pays_fee: Pays::No,
            })
        }

        /// Used by Acurast transmitters to submit a `state_merkle_root` at the specified `block` on the target chain.
        #[pallet::call_index(1)]
        #[pallet::weight(< T as Config<I>>::WeightInfo::submit_state_merkle_root())]
        pub fn submit_state_merkle_root(
            origin: OriginFor<T>,
            snapshot: T::TargetChainBlockNumber,
            state_merkle_root: T::TargetChainHash,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let expected_snapshot = Self::latest_snapshot();

            // Ensure merkle roots are submitted sequentially
            ensure!(
                snapshot == expected_snapshot,
                Error::<T, I>::UnexpectedSnapshot
            );

            let activity_window = <StateTransmitter<T, I>>::get(&who);
            let current_block = <frame_system::Pallet<T>>::block_number();
            // valid window is defined inclusive start_block, exclusive end_block
            ensure!(
                activity_window.start_block <= current_block
                    && current_block < activity_window.end_block,
                Error::<T, I>::SubmitOutsideTransmitterActivityWindow
            );

            // insert merkle root proposal since all checks passed
            // allows for constant-time validity checks
            let accepted = StateMerkleRootCount::<T, I>::mutate(
                &snapshot,
                &state_merkle_root,
                |submissions| {
                    // This can be improved once [let chains feature](https://github.com/rust-lang/rust/issues/53667) lands
                    if let Some(transmitters) = submissions {
                        if !transmitters.contains(&who) {
                            _ = transmitters.try_insert(who.clone());
                        }
                    } else {
                        let mut set =
                            BoundedBTreeSet::<T::AccountId, T::MaxTransmittersPerSnapshot>::new();
                        _ = set.try_insert(who.clone());
                        *submissions = Some(set);
                    }

                    let submissions_count = submissions
                        .as_ref()
                        .map_or(0usize, |transmitters| transmitters.len());
                    return submissions_count >= T::TransmissionQuorum::get().into();
                },
            );

            // Emit event to inform that the state merkle root has been sumitted
            Self::deposit_event(Event::StateMerkleRootSubmitted {
                source: who,
                snapshot,
                state_merkle_root,
            });

            if accepted {
                CurrentSnapshot::<T, I>::set(expected_snapshot + Self::current_transmission_rate());
                Self::deposit_event(Event::StateMerkleRootAccepted {
                    snapshot,
                    state_merkle_root,
                });
            }

            Ok(())
        }

        /// Used by any transmitter to submit a `state` that is at the specified `block` on the target chain.
        ///
        /// # Error behaviour
        ///
        /// We fail with a [`DispatchError`] if the given `proof` is invalid.
        /// Any error happening afterwards, while decoding the payload and triggering actions, emits an event informing about the error but does not fail the extrinsic.
        /// This is necessary to make [`MessageSequenceId`] update in any case.
        #[pallet::call_index(2)]
        #[pallet::weight(< T as Config<I>>::WeightInfo::submit_message())]
        pub fn submit_message(
            origin: OriginFor<T>,
            // The block number at which the state proof was generated.
            snapshot: T::TargetChainBlockNumber,
            // The state proof.
            proof: T::Proof,
        ) -> DispatchResultWithPostInfo {
            let _ = ensure_signed(origin)?;

            let derived_root = proof.calculate_root().map_err(|err| {
                log::debug!("Failed to validate proof: {:?}", &err);

                Error::<T, I>::ProofInvalid
            })?;

            if !Self::validate_state_merkle_root(snapshot, T::TargetChainHash::from(derived_root)) {
                return Err(Error::<T, I>::ProofDoesNotMatch)?;
            }

            let _message_id = Self::process_message_id(&proof)?;

            // don't fail extrinsic from here onwards
            if let Err(e) = Self::process_action(&proof) {
                Self::deposit_event(Event::MessageProcessed(e));
            } else {
                Self::deposit_event(Event::MessageProcessed(ProcessMessageResult::ActionSuccess));
            }

            Ok(().into())
        }

        /// Updates the target chain owner (contract address) in storage. Can only be called by a privileged/root account.
        #[pallet::call_index(3)]
        #[pallet::weight(< T as Config<I>>::WeightInfo::update_target_chain_owner())]
        pub fn update_target_chain_owner(
            origin: OriginFor<T>,
            owner: StateOwner,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Self::set_target_chain_owner(owner.clone());
            Self::deposit_event(Event::TargetChainOwnerUpdated { owner });
            Ok(())
        }

        /// Update the current snapshot being confirmed
        #[pallet::call_index(4)]
        #[pallet::weight(< T as Config<I>>::WeightInfo::update_current_snapshot())]
        pub fn update_current_snapshot(
            origin: OriginFor<T>,
            snapshot: T::TargetChainBlockNumber,
        ) -> DispatchResult {
            ensure_root(origin)?;
            CurrentSnapshot::<T, I>::set(snapshot);
            Ok(())
        }
    }

    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        /// Validates a state merkle root with respect to roots submitted by a quorum of transmitters.
        pub fn validate_state_merkle_root(
            block: T::TargetChainBlockNumber,
            state_merkle_root: T::TargetChainHash,
        ) -> bool {
            StateMerkleRootCount::<T, I>::get(&block, &state_merkle_root)
                .map_or(false, |submissions| {
                    submissions.len() >= T::TransmissionQuorum::get().into()
                })
        }

        /// Sets the target chain owner (contract address) in storage.
        pub fn set_target_chain_owner(owner: StateOwner) {
            <CurrentTargetChainOwner<T, I>>::set(owner);
        }

        /// Processes a message with `key` and `payload`.
        ///
        /// **When action processing fails, the message sequence increment above is still persisted, only side-effects produced by the action should be reverted**.
        /// See [`Self::process_action()`].
        fn process_message_id(proof: &T::Proof) -> Result<MessageIdentifier, Error<T, I>> {
            let message_id = proof.message_id().map_err(|err| {
                log::debug!("Could get message id: {:?}", err);
                #[cfg(test)]
                dbg!(err);

                Error::<T, I>::InvalidMessageId
            })?;

            ensure!(
                Self::message_seq_id() + 1 == message_id.into(),
                Error::<T, I>::MessageIdDoesNotMatch
            );
            <MessageSequenceId<T, I>>::set(message_id);

            Ok(message_id)
        }

        #[transactional]
        fn process_action(proof: &T::Proof) -> Result<(), ProcessMessageResult> {
            let action = proof
                .message()
                .map_err(|_| ProcessMessageResult::ParsingValueFailed)?;

            let raw_action: RawAction = (&action).into();
            T::ActionExecutor::execute(action)
                .map_err(|_| ProcessMessageResult::ActionFailed(raw_action))?;

            Ok(())
        }
    }
}
