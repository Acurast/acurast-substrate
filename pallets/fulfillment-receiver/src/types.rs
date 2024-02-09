use acurast_common::Script;
use frame_support::pallet_prelude::*;
use sp_std::prelude::*;

/// Structure representing a job fulfillment. It contains the script that generated the payload and the actual payload.
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq)]
pub struct Fulfillment {
    /// The script that generated the payload.
    pub script: Script,
    /// The output of a script.
    pub payload: Vec<u8>,
}
