use frame_support::{
	sp_runtime::{DispatchError, DispatchResult},
	traits::{
		fungible::{Balanced, Imbalance, Inspect},
		Get, IsType,
	},
};
use sp_std::{fmt, prelude::*};

use crate::{Attestation, AttestationChain, MetricInput, Version};

/// A bound that can be used to restrict length sequence types such as [`frame_support::BoundedVec`] appearing in types used in dispatchable functions.
///
/// Similar to [`frame_support::Parameter`] without encoding traits, since bounds are never encoded.
pub trait ParameterBound: Get<u32> + Clone + Eq + fmt::Debug + scale_info::TypeInfo {}
impl<T> ParameterBound for T where T: Get<u32> + Clone + Eq + fmt::Debug + scale_info::TypeInfo {}

pub trait ManagerIdProvider<AccountId, ManagerId> {
	fn create_manager_id(id: ManagerId, owner: &AccountId) -> DispatchResult;
	fn manager_id_for(owner: &AccountId) -> Result<ManagerId, DispatchError>;
	fn owner_for(manager_id: ManagerId) -> Result<AccountId, DispatchError>;
}

pub trait CommitmentIdProvider<AccountId, CommitmentId> {
	fn create_commitment_id(id: CommitmentId, owner: &AccountId) -> DispatchResult;
	fn commitment_id_for(owner: &AccountId) -> Result<CommitmentId, DispatchError>;
	fn owner_for(commitment_id: CommitmentId) -> Result<AccountId, DispatchError>;
}

/// A trait to describe hooks the `pallet_acruast_compute` provides.
pub trait ComputeHooks<AccountId, ManagerId, Balance> {
	/// Commits compute for current processor epoch by providing benchmarked results for a (sub)set of metrics.
	///
	/// **The caller has to ensure the passed processor is allowed to commit**.
	///
	/// Metrics are specified with the `pool_name` and an lookup will map the names to their corresponding `pool_id`.
	///
	/// # Errors
	///
	/// **Unknown pools are silently skipped.**
	fn commit(
		processor: &AccountId,
		manager: &(AccountId, ManagerId),
		metrics: &[MetricInput],
	) -> (Balance, bool, bool)
	where
		Balance: IsType<u128>;
}

impl<AccountId, ManagerId, Balance> ComputeHooks<AccountId, ManagerId, Balance> for () {
	fn commit(
		_processor: &AccountId,
		_manager: &(AccountId, ManagerId),
		_metrics: &[MetricInput],
	) -> (Balance, bool, bool)
	where
		Balance: IsType<u128>,
	{
		(0u128.into(), false, false)
	}
}

pub trait ProcessorVersionProvider<AccountId> {
	fn processor_version(processor: &AccountId) -> Option<Version>;
	fn min_version_for_reward(platform: u32) -> Option<Version>;
}

pub trait EnsureAttested<AccountId> {
	fn ensure_attested(processor: &AccountId) -> DispatchResult;
}

pub trait ManagerLookup {
	type AccountId;
	type ManagerId;

	fn lookup(processor: &Self::AccountId) -> Option<(Self::AccountId, Self::ManagerId)>;
	fn lookup_manager_id(processor: &Self::AccountId) -> Option<Self::ManagerId>;
}

pub trait AttestationValidator<AccountId> {
	fn validate(
		attestation_chain: &AttestationChain,
		account: &AccountId,
	) -> Result<Attestation, DispatchError>;
	fn validate_and_store(
		attestation_chain: AttestationChain,
		account: AccountId,
	) -> DispatchResult;
}

pub trait IsFundableCall<Call> {
	/// Whether `call` is eligible to be paid out of a manager's protocol-funded onboarding
	/// reserve (i.e. via `release_fee_funds`). This is the narrower "protocol funding" gate.
	fn is_fundable_call(call: &Call) -> bool;

	/// Whether `call` may be paid by the processor's manager at all (from the onboarding reserve
	/// first, then the manager's free balance). This is the wider "manager funding" gate and must
	/// be a superset of [`Self::is_fundable_call`]: every reserve-eligible call is also
	/// manager-fundable, but a manager may additionally sponsor calls that are not reserve-funded.
	///
	/// Any call that is not manager-fundable is paid by the submitting account itself, which is
	/// what prevents a paired processor from charging arbitrary runtime calls to its manager.
	fn is_manager_fundable_call(call: &Call) -> bool;
}

pub trait Slashable<AccountId> {
	type Currency: Balanced<AccountId>;

	fn slash(
		account: &AccountId,
		amount: BalanceFor<Self::Currency, AccountId>,
	) -> Option<ImbalanceFor<Self::Currency, AccountId>>;
}

pub type BalanceFor<Currency, AccountId> = <Currency as Inspect<AccountId>>::Balance;

pub type ImbalanceFor<Currency, AccountId> = Imbalance<
	BalanceFor<Currency, AccountId>,
	<Currency as Balanced<AccountId>>::OnDropCredit,
	<Currency as Balanced<AccountId>>::OnDropDebt,
>;

#[impl_trait_for_tuples::impl_for_tuples(10)]
pub trait OnProcessorUnpaired<AccountId> {
	fn processor_unpaired(processor: &AccountId, former_manager: &AccountId);
}
