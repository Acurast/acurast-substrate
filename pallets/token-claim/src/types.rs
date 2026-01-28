use frame_support::{pallet_prelude::*, traits::Currency};
use parity_scale_codec::{Decode, Encode};
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, Bounded, Convert, IdentifyAccount, Verify},
	RuntimeDebug,
};
use sp_std::prelude::*;

use crate::pallet::Config;

pub type BalanceFor<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

pub type ProcessedClaimFor<T> = ProcesssedClaim<
	<T as frame_system::Config>::AccountId,
	<T as Config>::Signature,
	BalanceFor<T>,
>;

pub type ClaimProofFor<T> =
	ClaimProof<<T as Config>::Signature, BalanceFor<T>, <T as frame_system::Config>::AccountId>;

pub type VestingInfoFor<T> = VestingInfo<
	BalanceFor<T>,
	frame_system::pallet_prelude::BlockNumberFor<T>,
	<T as frame_system::Config>::AccountId,
>;

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Eq, PartialEq, Clone, MaxEncodedLen)]
pub struct ProcesssedClaim<AccountId, Signature, Balance> {
	pub proof: ClaimProof<Signature, Balance, AccountId>,
	pub destination: AccountId,
}

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
)]
pub struct ClaimProof<Signature, Balance, AccountId> {
	pub amount: Balance,
	pub signature: Signature,
	_phantom: PhantomData<AccountId>,
}

impl<Signature, Balance, AccountId> ClaimProof<Signature, Balance, AccountId> {
	pub fn new(amount: Balance, signature: Signature) -> Self {
		Self { amount, signature, _phantom: Default::default() }
	}
}

impl<Signature, Balance, AccountId> ClaimProof<Signature, Balance, AccountId>
where
	Signature: Parameter + Member + Verify,
	Balance: Encode,
	AccountId: IsType<<<Signature as Verify>::Signer as IdentifyAccount>::AccountId> + Encode,
{
	pub fn validate(&self, account_id: &AccountId, signer: AccountId) -> bool {
		let message =
			[b"<Bytes>".to_vec(), account_id.encode(), self.amount.encode(), b"</Bytes>".to_vec()]
				.concat();
		self.signature.verify(message.as_ref(), &signer.into())
	}
}

/// Struct to encode the vesting schedule of an individual account.
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
)]
pub struct VestingInfo<Balance, BlockNumber, AccountId> {
	/// Initial claimer (could be different from destination).
	pub claimer: AccountId,
	/// Amount that gets unlocked every block after `starting_block`.
	pub per_block: Balance,
	/// Starting block for unlocking(vesting).
	pub starting_block: BlockNumber,

	/// Starting block for unlocking(vesting), updated to last vest operation.
	pub latest_vest: BlockNumber,
	/// Remaining amount to be vested according to schedule.
	pub remaining: Balance,
}

impl<Balance, BlockNumber, AccountId> VestingInfo<Balance, BlockNumber, AccountId>
where
	Balance: AtLeast32BitUnsigned + Copy,
	BlockNumber: AtLeast32BitUnsigned + Copy + Bounded,
{
	/// Vestable amount of remaining amount at block `n`.
	///
	/// Capped at [`Self::remaining`].
	pub fn vestable<BlockNumberToBalance: Convert<BlockNumber, Balance>>(
		&self,
		n: BlockNumber,
	) -> Balance {
		// Number of blocks that count toward vesting;
		// saturating to 0 when n < latest_vest.
		let vested_block_count = n.saturating_sub(self.latest_vest);
		let vested_block_count = BlockNumberToBalance::convert(vested_block_count);
		// Calculate amount vested so far, capped at remaining.
		let vested = vested_block_count.saturating_mul(self.per_block);
		// Cap at remaining to avoid overflow
		if vested > self.remaining {
			self.remaining
		} else {
			vested
		}
	}
}
