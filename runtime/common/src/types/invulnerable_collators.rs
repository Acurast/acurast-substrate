use core::marker::PhantomData;

use frame_support::traits::Get;
use sp_std::prelude::*;

pub struct InvulnerableCollators<Runtime>(PhantomData<Runtime>);
impl<Runtime> Get<Vec<<Runtime as frame_system::Config>::AccountId>>
	for InvulnerableCollators<Runtime>
where
	Runtime: pallet_collator_selection::Config,
{
	fn get() -> Vec<<Runtime as frame_system::Config>::AccountId> {
		pallet_collator_selection::Invulnerables::<Runtime>::get().into_inner()
	}
}
