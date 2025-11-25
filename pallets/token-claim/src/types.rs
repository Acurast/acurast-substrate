use frame_support::{
	pallet_prelude::*,
	traits::{Currency, VestedTransfer},
};
use parity_scale_codec::{Decode, Encode};
use sp_runtime::{
	traits::{IdentifyAccount, Verify},
	RuntimeDebug,
};
use sp_std::prelude::*;

use crate::Config;

pub type BalanceFor<T> = <<<T as Config>::VestedTransferer as VestedTransfer<
	<T as frame_system::Config>::AccountId,
>>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

pub type ProcessedClaimFor<T> = ProcesssedClaim<
	<T as frame_system::Config>::AccountId,
	<T as Config>::Signature,
	BalanceFor<T>,
>;

pub type ClaimProofFor<T> =
	ClaimProof<<T as Config>::Signature, BalanceFor<T>, <T as frame_system::Config>::AccountId>;

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
