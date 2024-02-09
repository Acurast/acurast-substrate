use frame_support::{
    pallet_prelude::DispatchResult,
    sp_runtime::{
        traits::{CheckedAdd, IdentifyAccount, Verify},
        DispatchError,
    },
    traits::IsType,
};

use crate::{
    Config, Error, LastManagerId, ManagedProcessors, ManagerIdProvider, Pallet,
    ProcessorToManagerIdIndex,
};

impl<T: Config> Pallet<T>
where
    T::AccountId: IsType<<<T::Proof as Verify>::Signer as IdentifyAccount>::AccountId>,
{
    /// Returns the manager account id (if any) for the given processor account.
    pub fn manager_for_processor(processor_account: &T::AccountId) -> Option<T::AccountId> {
        let id = Self::manager_id_for_processor(processor_account)?;
        <T::ManagerIdProvider as ManagerIdProvider<T>>::owner_for(id).ok()
    }

    /// Returns the manager id for the given manager account. If a manager id does not exists it is first created.
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
        if let Some(id) = Self::manager_id_for_processor(&processor_account) {
            if id == manager_id {
                return Err(Error::<T>::ProcessorAlreadyPaired)?;
            }
            return Err(Error::<T>::ProcessorPairedWithAnotherManager)?;
        }
        <ManagedProcessors<T>>::insert(manager_id, &processor_account, ());
        <ProcessorToManagerIdIndex<T>>::insert(&processor_account, manager_id);

        Ok(())
    }

    /// Removes the pairing between a processor account and manager id. It fails if the processor account is paired
    /// with a different manager id.
    pub fn do_remove_processor_manager_pairing(
        processor_account: &T::AccountId,
        manager_id: T::ManagerId,
    ) -> DispatchResult {
        if let Some(id) = Self::manager_id_for_processor(processor_account) {
            if id != manager_id {
                return Err(Error::<T>::ProcessorPairedWithAnotherManager)?;
            }
            <ManagedProcessors<T>>::remove(manager_id, &processor_account);
            <ProcessorToManagerIdIndex<T>>::remove(&processor_account);
        }

        Ok(())
    }
}
