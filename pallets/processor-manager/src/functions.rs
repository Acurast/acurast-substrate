use acurast_common::{
	AccountLookup, AttestationValidator, IsFundableCall, ManagerIdProvider,
	ProcessorVersionProvider, Version,
};
use frame_support::{
	pallet_prelude::{DispatchResult, Zero},
	sp_runtime::{
		traits::{CheckedAdd, IdentifyAccount, Saturating, Verify},
		DispatchError, SaturatedConversion,
	},
	traits::{
		fungible::{Inspect, InspectHold, MutateHold},
		tokens::{Fortitude, Precision, Preservation, WithdrawConsequence},
		Currency, ExistenceRequirement, IsSubType, IsType,
	},
};

use crate::{
	BalanceFor, Call, Config, Error, HoldReason, LastManagerId, ManagedProcessors,
	OnboardingProvider, Pallet, ProcessorPairingFor, ProcessorRewardDistributionWindow,
	ProcessorToManagerIdIndex, RewardDistributionWindow,
};

impl<T: Config> Pallet<T> {
	/// Returns the manager id for the given manager account. If a manager id does not exist it is first created.
	pub fn do_get_or_create_manager_id(
		manager: &T::AccountId,
	) -> Result<(T::ManagerId, bool), DispatchError> {
		T::ManagerIdProvider::manager_id_for(manager)
			.map(|id| (id, false))
			.or_else::<DispatchError, _>(|_| {
				let id = <LastManagerId<T>>::get()
					.unwrap_or(0u128.into())
					.checked_add(&1u128.into())
					.ok_or(Error::<T>::FailedToCreateManagerId)?;

				T::ManagerIdProvider::create_manager_id(id, manager)?;
				<LastManagerId<T>>::set(Some(id));

				Ok((id, true))
			})
	}

	/// Adds a pairing between the given processor account and manager id. It fails if the manager id does not exists of
	/// if the processor account was already paired.
	pub fn do_add_processor_manager_pairing(
		processor_account: &T::AccountId,
		manager_id: T::ManagerId,
	) -> DispatchResult {
		if let Some(id) = Self::manager_id_for_processor(processor_account) {
			if id == manager_id {
				return Err(Error::<T>::ProcessorAlreadyPaired)?;
			}
			return Err(Error::<T>::ProcessorPairedWithAnotherManager)?;
		}
		<ManagedProcessors<T>>::insert(manager_id, processor_account, ());
		<ProcessorToManagerIdIndex<T>>::insert(processor_account, manager_id);

		Ok(())
	}

	/// Removes the pairing between a processor account and manager id. It fails if the processor account is paired
	/// with a different manager id.
	pub fn do_remove_processor_manager_pairing(
		processor_account: &T::AccountId,
		manager: &T::AccountId,
	) -> DispatchResult {
		let id = Self::ensure_managed(manager, processor_account)?;
		<ManagedProcessors<T>>::remove(id, processor_account);
		<ProcessorToManagerIdIndex<T>>::remove(processor_account);
		Ok(())
	}

	pub(crate) fn do_reward_distribution(processor: &T::AccountId) -> Option<BalanceFor<T>> {
		let Some(distribution_settings) = Self::processor_reward_distribution_settings() else {
			<ProcessorRewardDistributionWindow<T>>::remove(processor);
			return None;
		};
		if distribution_settings.reward_per_distribution.is_zero() {
			<ProcessorRewardDistributionWindow<T>>::remove(processor);
			return None;
		}
		let Some(manager) = T::EligibleRewardAccountLookup::lookup(processor) else {
			<ProcessorRewardDistributionWindow<T>>::remove(processor);
			return None;
		};
		let current_block_number: u32 = <frame_system::Pallet<T>>::block_number().saturated_into();
		let Some(distribution_window) = Self::processor_reward_distribution_window(processor)
		else {
			<ProcessorRewardDistributionWindow<T>>::insert(
				processor,
				RewardDistributionWindow::new(current_block_number, &distribution_settings),
			);
			return None;
		};

		let progress = current_block_number.saturating_sub(distribution_window.start);
		if progress < distribution_window.window_length {
			<ProcessorRewardDistributionWindow<T>>::insert(processor, distribution_window.next());
			return None;
		}

		let mut distributed_amount: Option<BalanceFor<T>> = None;
		let buffer = progress.saturating_sub(distribution_window.window_length);
		if buffer <= distribution_window.tollerance
			&& (distribution_window.heartbeats + 1) >= distribution_window.min_heartbeats
		{
			let result = T::Currency::transfer(
				&distribution_settings.distributor_account,
				&manager,
				distribution_settings.reward_per_distribution,
				ExistenceRequirement::KeepAlive,
			);
			if result.is_ok() {
				distributed_amount = Some(distribution_settings.reward_per_distribution)
			}
		}
		<ProcessorRewardDistributionWindow<T>>::insert(
			processor,
			RewardDistributionWindow::new(current_block_number, &distribution_settings),
		);
		distributed_amount
	}

	pub fn ensure_managed(
		manager: &T::AccountId,
		processor: &T::AccountId,
	) -> Result<T::ManagerId, DispatchError> {
		let processor_manager_id =
			Self::manager_id_for_processor(processor).ok_or(Error::<T>::ProcessorHasNoManager)?;

		let processor_manager = T::ManagerIdProvider::owner_for(processor_manager_id)?;

		if manager != &processor_manager {
			return Err(Error::<T>::ProcessorPairedWithAnotherManager)?;
		}

		Ok(processor_manager_id)
	}
}

impl<T: Config> Pallet<T>
where
	T::AccountId: IsType<<<T::Proof as Verify>::Signer as IdentifyAccount>::AccountId>,
{
	pub fn do_validate_pairing(
		pairing: &ProcessorPairingFor<T>,
		is_multi: bool,
	) -> Result<Option<T::Counter>, DispatchError> {
		let mut new_counter: Option<T::Counter> = None;
		if !pairing.validate_timestamp::<T>() {
			#[cfg(not(feature = "runtime-benchmarks"))]
			return Err(Error::<T>::PairingProofExpired)?;
		}
		if is_multi {
			if !pairing.multi_validate_signature::<T>(&pairing.account) {
				#[cfg(not(feature = "runtime-benchmarks"))]
				return Err(Error::<T>::InvalidPairingProof)?;
			}
		} else {
			let counter = Self::counter_for_manager(&pairing.account)
				.unwrap_or(0u8.into())
				.checked_add(&1u8.into());
			if let Some(counter) = counter {
				if !pairing.validate_signature::<T>(&pairing.account, counter) {
					#[cfg(not(feature = "runtime-benchmarks"))]
					return Err(Error::<T>::InvalidPairingProof)?;
				}
				new_counter = Some(counter);
			} else {
				return Err(Error::<T>::CounterOverflow)?;
			}
		}
		Ok(new_counter)
	}
}

impl<T: Config> ProcessorVersionProvider<T::AccountId> for Pallet<T> {
	fn processor_version(processor: &T::AccountId) -> Option<Version> {
		Self::processor_version(processor)
	}

	fn min_version_for_reward(platform: u32) -> Option<Version> {
		let build_number = Self::processor_min_version_for_reward(platform);
		build_number.map(|build_number| Version { platform, build_number })
	}
}

impl<T: Config> AccountLookup<T::AccountId> for Pallet<T> {
	fn lookup(processor: &T::AccountId) -> Option<T::AccountId> {
		let manager_id = Self::manager_id_for_processor(processor)?;
		T::ManagerIdProvider::owner_for(manager_id).ok()
	}
}

impl<T: Config> OnboardingProvider<T> for Pallet<T>
where
	T::AccountId: IsType<<<T::Proof as Verify>::Signer as IdentifyAccount>::AccountId>,
	T::RuntimeCall: IsSubType<Call<T>>,
	BalanceFor<T>: IsType<u128>,
{
	fn validate_pairing(pairing: &ProcessorPairingFor<T>, is_multi: bool) -> DispatchResult {
		_ = Self::do_validate_pairing(pairing, is_multi)?;
		Ok(())
	}

	fn validate_attestation(
		attestation_chain: &acurast_common::AttestationChain,
		account: &T::AccountId,
	) -> DispatchResult {
		_ = T::AttestationHandler::validate(attestation_chain, account)?;
		Ok(())
	}

	fn can_fund_processor_onboarding(processor: &T::AccountId) -> bool {
		if Self::manager_id_for_processor(processor).is_some() {
			return false;
		}
		let Some(settings) = Self::processor_onboarding_settings() else {
			return false;
		};
		if settings.funds.is_zero() {
			return false;
		}
		let consequences = T::Currency::can_withdraw(&settings.funds_account, settings.funds);
		matches!(consequences, WithdrawConsequence::Success)
	}

	fn fund(account: &T::AccountId) -> DispatchResult {
		let Some(settings) = Self::processor_onboarding_settings() else {
			return Err(Error::<T>::OnboardingSettingsNotSet)?;
		};
		T::Currency::transfer(
			&settings.funds_account,
			account,
			settings.funds,
			ExistenceRequirement::KeepAlive,
		)?;
		let amount_to_hold = settings.funds.min(T::Currency::reducible_balance(
			account,
			Preservation::Protect,
			Fortitude::Polite,
		));
		T::Currency::hold(&HoldReason::Onboarding.into(), account, amount_to_hold)
	}

	fn can_cover_fee(account: &T::AccountId, fee: BalanceFor<T>) -> (bool, BalanceFor<T>) {
		let available = T::Currency::balance_on_hold(&HoldReason::Onboarding.into(), account);
		let missing = fee.saturating_sub(available);
		(available >= fee, missing)
	}

	fn release_fee_funds(account: &T::AccountId, fee: BalanceFor<T>) {
		_ = T::Currency::release(
			&HoldReason::Onboarding.into(),
			account,
			fee,
			Precision::BestEffort,
		);
	}

	fn pairing_for_call(
		call: &<T>::RuntimeCall,
	) -> Option<(&ProcessorPairingFor<T>, bool, Option<&acurast_common::AttestationChain>)> {
		let call = T::RuntimeCall::is_sub_type(call)?;
		match call {
			Call::pair_with_manager { pairing } => Some((pairing, false, None)),
			Call::multi_pair_with_manager { pairing } => Some((pairing, true, None)),
			Call::onboard { pairing, multi, attestation_chain } => {
				Some((pairing, *multi, Some(attestation_chain)))
			},
			_ => None,
		}
	}

	fn is_funding_call(call: &<T>::RuntimeCall) -> bool {
		let Some(call) = T::RuntimeCall::is_sub_type(call) else {
			return false;
		};
		matches!(call, Call::onboard { .. })
	}

	fn fee_payer(account: &T::AccountId, call: &T::RuntimeCall) -> T::AccountId {
		let mut manager = Self::lookup(account);

		if manager.is_none() {
			if let Some((pairing, _, _)) = Self::pairing_for_call(call) {
				manager = Some(pairing.account.clone());
			}
		}

		manager.unwrap_or(account.clone())
	}
}

impl<T: Config> IsFundableCall<T::RuntimeCall> for Pallet<T>
where
	T::RuntimeCall: IsSubType<Call<T>>,
	BalanceFor<T>: IsType<u128>,
	T::AccountId: IsType<<<T::Proof as Verify>::Signer as IdentifyAccount>::AccountId>,
{
	fn is_fundable_call(call: &T::RuntimeCall) -> bool {
		let Some(call) = T::RuntimeCall::is_sub_type(call) else {
			return false;
		};
		matches!(
			call,
			Call::heartbeat_with_metrics { .. }
				| Call::heartbeat_with_version { .. }
				| Call::onboard { .. }
				| Call::multi_pair_with_manager { .. }
				| Call::pair_with_manager { .. }
		)
	}
}
