use crate::{MessageIdentifier, ParsedAction};
use frame_support::weights::Weight;
use sp_std::fmt::Debug;

pub trait Proof<T, I: 'static>
where
    T: crate::pallet::Config<I>,
{
    type Error: Debug;

    fn calculate_root(self: &Self) -> Result<[u8; 32], Self::Error>;
    fn message_id(self: &Self) -> Result<MessageIdentifier, Self::Error>;
    fn message(self: &Self) -> Result<ParsedAction<T>, Self::Error>;
}

/// Weight functions needed for pallet_acurast_hyperdrive.
pub trait WeightInfo {
    fn update_state_transmitters(l: u32) -> Weight;
    fn submit_state_merkle_root() -> Weight;
    fn submit_message() -> Weight;
    fn update_target_chain_owner() -> Weight;
    fn update_current_snapshot() -> Weight;
}
